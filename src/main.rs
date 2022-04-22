use gluon::{ThreadExt, new_vm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tokio::{
  sync::{RwLock, mpsc::{unbounded_channel, UnboundedSender}},
  task
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Filter, ws::{Message, WebSocket, Ws}};
use futures_util::{SinkExt, StreamExt};

#[cfg(windows)]
mod win_tray;
#[cfg(windows)]
use crate::win_tray::Tray;

static NEXT_WSP_ID: AtomicUsize = AtomicUsize::new(1);

type WsPool = Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() {
  // arc config
  let wspool = WsPool::default();
  let wspool_q = wspool.clone();
  // quit channel
  let (tx, rx) = unbounded_channel();
  let mut  rx = UnboundedReceiverStream::new(rx);
  // if on windows, get the systray
  #[cfg(windows)]
  { Tray::new(tx.clone()).unwrap(); }
  // routes
  let quit = warp::path("quit").map(move || {
    let tx = tx.clone();
    tx.send(()).unwrap();
    "quit ok"
  });
  let wspool_w = warp::any().map(move || wspool.clone());
  let ws = warp::path("ws")
    .and(warp::ws())
    .and(wspool_w)
    .map(|ws: Ws, wspool_w|
      ws.on_upgrade(move |socket| ws_handler(socket, wspool_w))
    );
  let stop = warp::path("stop").map(move || {"stop"});
  let dir = warp::fs::dir("./dist");
  let others = warp::any().and(warp::fs::file("./dist/chat.html"));
  let routes = quit.or(stop).or(ws).or(dir).or(others);
  // launching server
  let (_, svr) = warp::serve(routes)
    .bind_with_graceful_shutdown(
      ([127, 0, 0, 1], 3030),
      async move {
        rx.next().await;
        let msg = WsMessage::new(WsMessageType::Command, String::from("close"));
        let msg = serde_json::to_string(&msg).unwrap();
        wspool_q.read().await.values().for_each(|u_tx|
          u_tx.send(Message::text(&msg)).unwrap()
        );
      }
    );
  //#[cfg(windows)]
  //{ crate::win_tray::url_open(); }
  let _ = svr.await;
}

#[derive(Deserialize, Serialize)]
enum WsMessageType {
  Command,
  Text
}

#[derive(Deserialize, Serialize)]
struct WsMessage {
  msg_type: WsMessageType,
  content: String
}

impl WsMessage {
  fn new(msg_type: WsMessageType, content: String) -> Self {
    Self { msg_type, content }
  }
}

async fn ws_handler(socket: WebSocket, wspool: WsPool) {
  // config
  let ws_id = NEXT_WSP_ID.fetch_add(1, Ordering::Relaxed);
  let (mut ws_tx, mut ws_rx) = socket.split();
  let (u_tx, u_rx) = unbounded_channel();
  let mut u_rx = UnboundedReceiverStream::new(u_rx);
  let gl_vm = new_vm();
  // saying hello to everyone
  let msg = WsMessage::new(
    WsMessageType::Command, format!("add:{}", ws_id));
  let msg = Message::text(&serde_json::to_string(&msg).unwrap());
  wspool.read().await.values()
    .for_each(|u_tx| u_tx.send(msg.clone()).unwrap_or_else(|_| ()));
  // saving id in the hashmap
  wspool.write().await.insert(ws_id, u_tx.clone());
  // listening from other ws
  task::spawn(async move { while let Some(msg) = u_rx.next().await {
    match ws_tx.send(msg).await {
      Ok(_) => (),
      Err(_e) => { /*eprintln!("error (#{}): {}", ws_id, e);*/ break; }
    }
  }});
  // sending id to the user
  let msg = WsMessage::new(
    WsMessageType::Command, format!("#{}", ws_id));
  let msg = Message::text(&serde_json::to_string(&msg).unwrap());
  u_tx.send(msg).unwrap();
  // dialog loop
  while let Some(res) = ws_rx.next().await {
    let msg = match res {
      Ok(msg) => {
        if msg.is_close() {
          let msg = Message::text(&serde_json::to_string(
            &WsMessage::new(WsMessageType::Command, format!("quit:{}", ws_id))
          ).unwrap());
          wspool.read().await.values()
            .for_each(|u_tx| u_tx.send(msg.clone()).unwrap_or_else(|_| ()));
          break;
        } else { msg.to_str().unwrap().to_string() }
      }
      Err(e) => { eprintln!("ws err: {}", e); break; }
    };
    let res: WsMessage = serde_json::from_str(&msg).unwrap();
    match res.msg_type {
      WsMessageType::Command => {
        if res.content == "populate" {
          let reader = wspool.read().await;
          let mut list = reader.keys().filter(|k| *k != &ws_id)
            .collect::<Vec<_>>();
          list.sort();
          let msg = WsMessage::new(WsMessageType::Command,
            format!("pop:{}", serde_json::to_string(&list).unwrap()));
          let msg = Message::text(&serde_json::to_string(&msg).unwrap());
          u_tx.send(msg).unwrap_or_else(|_| ());
        }
      }
      WsMessageType::Text => {
        //
        // TODO
        //
        //println!("Text: {}", res.content);
        //
        gl_vm.load_file("scripts/parse.glu").unwrap();
        //
        //
        let tr = res.content;
        //
        let msg = WsMessage::new(WsMessageType::Text,
          format!("User #{}: {}", ws_id, tr));
        let msg = Message::text(serde_json::to_string(&msg).unwrap());
        let reader = wspool.read().await;
        for (_, u_tx) in reader.iter().filter(|(id, _)| *id != &ws_id) {
          u_tx.send(msg.clone()).unwrap_or_else(|_| ());
        }
      }
    }
  }
  // bye
  wspool.write().await.remove(&ws_id);
}
