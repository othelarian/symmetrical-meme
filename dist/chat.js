var ws;
var chat_zone;
var inp;

function initchat() {
  chat_zone = document.getElementById("chat_zone");
  inp = document.getElementById("inp");
  inp.addEventListener("keyup",
    (evt) => { if (evt.code == "Enter") {send_msg()}});
  ws = new WebSocket("ws://localhost:3030/ws");
  ws.onmessage = handle_msg;
  ws.onopen = function(_evt) {
    ws.send(JSON.stringify({"msg_type":"Command","content":"populate"}));
  }
}

function handle_msg(evt) {
  let msg = JSON.parse(evt.data);
  switch (msg.msg_type) {
    case "Command":
      if (msg.content == "close") {
        add_entry("end", "Server disconnected");
        ws.close();
        let r = new XMLHttpRequest();
        r.open("GET", "http://localhost:3030/stop", false);
        try { r.send(); } catch { console.log("failed"); }
      } else if (msg.content[0] == "#") {
        document.getElementById("user_id").innerText = msg.content[1];
        add_entry("cmd", `Welcome! You're user #${msg.content[1]}`);
      } else if (msg.content.substring(0, 3) == "add") {
        let user_id = msg.content.split(":")[1];
        add_entry("cmd", `User #${user_id} joined the chat`);
      } else if (msg.content.substring(0, 4) == "quit") {
        let user_id = msg.content.split(":")[1];
        add_entry("cmd", `User #${user_id} leaved the chat`);
      } else if (msg.content.substring(0, 3) == "pop") {
        let p = JSON.parse(msg.content.split(":")[1]);
        if (p.length) {
          p = p.map((u) => "user "+u).join(", ");
          add_entry("cmd", `The other user are: ${p}`);
        } else {
          add_entry("cmd", "You're alone here :(");
        }
      }
      break;
    case "Text":
      add_entry("", msg.content);
      break;
  }
}

function add_entry(e_class, text) {
  let d = document.createElement("div");
  d.setAttribute("class", e_class);
  d.innerText = text;
  chat_zone.append(d);
}

function send_msg() {
  let v = inp.value;
  if (v != "") {
    add_entry("msg", `You: ${v}`);
    let msg= {"msg_type": "Text", "content": v};
    ws.send(JSON.stringify(msg));
    inp.value = "";
  }
}
