// The board: the Rust side watches the open file and sends its text on each
// change; this script draws it with marked and KaTeX.

import { drawBoard } from "./math.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const pane = document.getElementById("board-pane");
const board = document.getElementById("board");
const note = document.getElementById("note");

let shownPath = null;
let shownReady = false;
let shownVersion = -1;

function atBottom() {
  return pane.scrollTop > 0 && pane.scrollTop + pane.clientHeight >= pane.scrollHeight - 4;
}

function show(snapshot) {
  if (snapshot.version <= shownVersion) return;
  shownVersion = snapshot.version;
  const samePath = snapshot.path === shownPath;
  const becameReady = snapshot.ready !== shownReady;
  shownPath = snapshot.path;
  shownReady = snapshot.ready;
  // The conversation belongs to the board's folder: load it for a new
  // board, and once a project on another host has connected.
  if (!samePath || becameReady) window.dispatchEvent(new CustomEvent("board-path"));

  if (!snapshot.path) {
    note.textContent = "Otwórz plik Markdown: Plik → Otwórz (⌘O).";
    note.hidden = false;
    return;
  }

  note.textContent = snapshot.error ? `${snapshot.error} ${snapshot.path}` : "";
  note.hidden = !snapshot.error;

  if (snapshot.text == null) {
    if (!samePath) board.innerHTML = "";
    return;
  }

  // The agent adds steps at the end: a reader at the bottom follows them.
  // Anyone else keeps their place.
  const followEnd = samePath && atBottom();
  const y = samePath ? pane.scrollTop : 0;
  drawBoard(board, snapshot.text);
  pane.scrollTop = followEnd ? pane.scrollHeight : y;
}

await listen("board", (event) => show(event.payload));
show(await invoke("board_current"));
