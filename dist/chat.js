var ws;
function initchat() {
  //
  // TODO: préparation du formulaire
  //
  //
  // websocket config
  ws = new WebSocket("ws://localhost:3030/ws");
  ws.onopen = function(evt) {
    //
    console.log("ws opened");
    console.log(evt);
    //
  }
  ws.onmessage = function(evt) {
    //
    //
  }
  ws.onclose = function(_evt) {
    console.log("ws closed");
  }
}
