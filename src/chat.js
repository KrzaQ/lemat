// The conversation panel: the messages, the agent's work folded above each
// answer, and the text field. The Rust side runs the agent and sends each
// new history entry as an event.

import { drawMarkdown } from "./math.js";
import { workSummary } from "./words.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const messages = document.getElementById("messages");
const input = document.getElementById("input");
const send = document.getElementById("send");
const stop = document.getElementById("stop");

let live = null;

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

function nearEnd() {
  return messages.scrollTop + messages.clientHeight >= messages.scrollHeight - 40;
}

function toEnd() {
  messages.scrollTop = messages.scrollHeight;
}

// The work stays folded: the answer matters, the work explains it on click.
function workView(work, running) {
  const details = element("details", "work");
  details.append(element("summary", null, workSummary(work, running)));
  if (work.thinking) details.append(element("p", "thinking", work.thinking));
  if (work.steps?.length) {
    const list = element("ol", "steps");
    for (const step of work.steps) {
      const item = element("li", `step ${step.state}`);
      item.append(element("span", "name", step.name));
      if (step.detail) item.append(element("span", "detail", ` ${step.detail}`));
      if (step.result) item.append(element("div", "result", step.result));
      list.append(item);
    }
    details.append(list);
  }
  return details;
}

function hasWork(work) {
  return work && (work.thinking || work.steps?.length);
}

function entryView(entry) {
  switch (entry.kind) {
    case "user":
      return element("div", "message user", entry.text);
    case "agent": {
      const node = element("div", "message agent");
      if (hasWork(entry.work)) node.append(workView(entry.work, false));
      const answer = element("div", "answer");
      drawMarkdown(answer, entry.text);
      node.append(answer);
      return node;
    }
    case "error": {
      const node = element("div", "message agent");
      if (hasWork(entry.work)) node.append(workView(entry.work, false));
      node.append(element("div", "error", entry.text));
      return node;
    }
    default:
      return null;
  }
}

function add(entry) {
  const follow = nearEnd();
  const view = entryView(entry);
  if (!view) return;
  if (entry.kind !== "user") clearLive();
  messages.insertBefore(view, live);
  if (follow || entry.kind === "user") toEnd();
}

function clearLive() {
  live?.remove();
  live = null;
}

function showProgress(work) {
  const follow = nearEnd();
  const open = live?.querySelector("details")?.open ?? false;
  const view = element("div", "message agent live");
  const details = workView(work, true);
  details.open = open;
  view.append(details);
  if (live) live.replaceWith(view);
  else messages.append(view);
  live = view;
  if (follow) toEnd();
}

function setRunning(running) {
  stop.hidden = !running;
  send.hidden = running;
  if (running && !live) showProgress({});
  if (!running) clearLive();
}

async function load() {
  const state = await invoke("chat_state");
  messages.replaceChildren();
  live = null;
  input.disabled = !state;
  input.placeholder = state
    ? "Powiedz, co dalej… (Enter wysyła, Esc zatrzymuje)"
    : "Najpierw otwórz plik tablicy.";
  for (const entry of state?.entries ?? []) add(entry);
  setRunning(state?.running ?? false);
  toEnd();
}

async function submit() {
  const text = input.value.trim();
  if (!text) return;
  input.value = "";
  try {
    await invoke("chat_send", { text });
  } catch (error) {
    add({ kind: "error", text: String(error), work: {} });
  }
}

input.addEventListener("keydown", (event) => {
  // Enter sends; Shift+Enter starts a new line. An input method that is
  // still composing a word keeps its Enter.
  if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
    event.preventDefault();
    submit();
  }
});
send.addEventListener("click", submit);
stop.addEventListener("click", () => invoke("chat_stop"));
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !stop.hidden) invoke("chat_stop");
});

// Dictation types into the focused field, so the field keeps the focus.
window.addEventListener("focus", () => {
  if (!input.disabled) input.focus();
});

await listen("chat", (event) => add(event.payload));
await listen("chat-progress", (event) => showProgress(event.payload));
await listen("chat-running", (event) => setRunning(event.payload));
window.addEventListener("board-path", load);
await load();
input.focus();
