// The board: the Rust side watches the open file and sends its text on each
// change; this script draws it with marked and KaTeX.

import { toHtml } from "./render.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const board = document.getElementById("board");
const note = document.getElementById("note");

let shownPath = null;
let shownVersion = -1;

function draw(text) {
  board.innerHTML = toHtml(text, (markdown) => marked.parse(markdown));
  renderMathInElement(board, {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
    ],
    throwOnError: false,
  });
}

function atBottom() {
  const page = document.documentElement;
  return window.scrollY > 0 && window.scrollY + window.innerHeight >= page.scrollHeight - 4;
}

function show(snapshot) {
  if (snapshot.version <= shownVersion) return;
  shownVersion = snapshot.version;
  const samePath = snapshot.path === shownPath;
  shownPath = snapshot.path;

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
  const y = samePath ? window.scrollY : 0;
  draw(snapshot.text);
  window.scrollTo(0, followEnd ? document.documentElement.scrollHeight : y);
}

await listen("board", (event) => show(event.payload));
show(await invoke("board_current"));
