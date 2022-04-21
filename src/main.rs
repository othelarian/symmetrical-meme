use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tokio::{
  sync::{RwLock, mpsc::{unbounded_channel, UnboundedSender}},
  task
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Filter, ws::{Message, WebSocket, Ws}};
use futures_util::{SinkExt, StreamExt, TryFutureExt};

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
  let wspool = warp::any().map(move || wspool.clone());
  let ws = warp::path("ws")
    .and(warp::ws())
    .and(wspool)
    .map(|ws: Ws, wspool|
      ws.on_upgrade(move |socket| ws_handler(socket, wspool))
    );
  let dir = warp::fs::dir("./dist");
  let others = warp::any().and(warp::fs::file("./dist/chat.html"));
  let routes = quit.or(ws).or(dir).or(others);
  // launching server
  let (_, svr) = warp::serve(routes)
    .bind_with_graceful_shutdown(
      ([127, 0, 0, 1], 3030),
      async move {
        rx.next().await;
        //
        // TODO: ext-ce que c'est ici qu'on traite les ws ?
        println!("shutdown activated");
        //
      }
    );
  //#[cfg(windows)]
  //{ crate::win_tray::url_open(); }
  let _ = svr.await;
}

async fn ws_handler(socket: WebSocket, wspool: WsPool) {
  let ws_id = NEXT_WSP_ID.fetch_add(1, Ordering::Relaxed);
  let (mut ws_tx, mut ws_rx) = socket.split();
  let (u_tx, u_rx) = unbounded_channel();
  let mut u_rx = UnboundedReceiverStream::new(u_rx);
  wspool.write().await.insert(ws_id, u_tx.clone());
  //
  // TODO: envoi d'un message à tout les ws
  //
  //
  //
  task::spawn(async move { while let Some(msg) = u_rx.next().await {
    ws_tx.send(msg).unwrap_or_else(|e| eprintln!("ws error: {}", e)).await;
  }});
  while let Some(msg) = ws_rx.next().await {
    //
    println!("ws used");
    //
    //
  }
  wspool.write().await.remove(&ws_id);
}
