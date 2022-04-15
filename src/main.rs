use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;
use futures_util::StreamExt;//TryFutureExt;


#[tokio::main]
async fn main() {
  //
  let (tx, rx) = unbounded_channel();
  let mut  rx = UnboundedReceiverStream::new(rx);
  //
  //
  let quit = warp::path("quit").map(move || {
    let tx = tx.clone();
    tx.send(()).unwrap();
    "quit ok"
  });
  let all = warp::any().map(|| "Hello, world");
  let routes = quit.or(all);
  //
  //
  //
  //
  let (_, svr) = warp::serve(routes)
    .bind_with_graceful_shutdown(
      ([127, 0, 0, 1], 3030),
      async move {
        //
        // TODO
        //
        match rx.next().await {
          Some(_) => println!("pk"),
          None => eprintln!("err")
        }
      }
    );
  //
  //
  let _ = svr.await;
  //
  // TODO: systray ici ?
  //
}

fn ws_handler() {
  //
  // TODO
  //
}
