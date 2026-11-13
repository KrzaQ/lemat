// The conversation panel: the messages, the agent's work folded above each
// answer, and the text field. The Rust side runs the agent and sends each
// new history entry as an event.

import { drawMarkdown } from "./math.js";
import { metaLine, workSummary } from "./words.js";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const messages = document.getElementById("messages");
const input = document.getElementById("input");
const send = document.getElementById("send");
const stop = document.getElementById("stop");
const engine = document.getElementById("engine");
const model = document.getElementById("model");
const models = document.getElementById("models");

let live = null;

// The agent's question that waits for an answer, with the option the
// arrows point at and the options ticked so far.
const dialog = document.getElementById("question");
let asked = null;

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
function workView(work, running, by) {
  const details = element("details", "work");
  details.append(element("summary", null, running ? workSummary(work, true) : metaLine(by, work)));
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
      if (hasWork(entry.work)) node.append(workView(entry.work, false, entry.by));
      else if (entry.by) node.append(element("div", "work", entry.by));
      const answer = element("div", "answer");
      drawMarkdown(answer, entry.text);
      node.append(answer);
      return node;
    }
    case "question": {
      const node = element("div", "message asked");
      node.append(element("div", "work", `Pytanie: ${entry.text}`));
      if (entry.detail) node.append(element("code", "detail", entry.detail));
      node.append(element("div", "said", `Odpowiedź: ${entry.answer}`));
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

function drawQuestion() {
  const q = asked.question;
  dialog.querySelector(".title").textContent = q.tool
    ? `Agent chce użyć narzędzia ${q.tool}`
    : "Agent pyta";
  dialog.querySelector(".text").textContent = q.text;
  const detail = dialog.querySelector(".detail");
  detail.textContent = q.detail;
  detail.hidden = !q.detail;
  const list = dialog.querySelector(".options");
  list.replaceChildren(
    ...q.options.map((option, i) => {
      const item = element("li", null, option);
      item.classList.toggle("current", i === asked.current);
      item.classList.toggle("ticked", asked.ticked.has(i));
      item.addEventListener("click", () => {
        asked.current = i;
        if (q.multiple) tick();
        else answer("chose");
      });
      return item;
    }),
  );
  list.hidden = q.options.length === 0;
  const choosing = q.options.length > 0;
  dialog.querySelector(".yes").textContent = choosing ? "Wybierz (Enter)" : "Tak (Enter)";
  dialog.querySelector(".no").textContent = choosing ? "Anuluj (Esc)" : "Nie (Esc)";
  dialog.querySelector(".hint").textContent = !choosing
    ? ""
    : q.multiple
      ? "↑↓ wybór · Spacja zaznacza"
      : "↑↓ wybór";
}

function showQuestion(question) {
  asked = question ? { question, current: 0, ticked: new Set() } : null;
  dialog.hidden = !asked;
  if (asked) {
    drawQuestion();
    dialog.scrollIntoView({ block: "nearest" });
  }
}

function tick() {
  if (asked.ticked.has(asked.current)) asked.ticked.delete(asked.current);
  else asked.ticked.add(asked.current);
  drawQuestion();
}

function answer(kind) {
  if (!asked) return;
  const q = asked.question;
  let chosen = [];
  if (kind === "yes" && q.options.length > 0) {
    kind = "chose";
  }
  if (kind === "chose") {
    chosen = q.multiple && asked.ticked.size > 0 ? [...asked.ticked].sort((a, b) => a - b) : [asked.current];
  }
  showQuestion(null);
  invoke("chat_answer", { kind, chosen });
  input.focus();
}

dialog.querySelector(".yes").addEventListener("click", () => answer("yes"));
dialog.querySelector(".no").addEventListener("click", () => answer("no"));

// While a question waits, its keys come first: a game pad sends A as
// Return and B as Escape, so these must answer the dialog, not send a
// message or stop the agent.
window.addEventListener(
  "keydown",
  (event) => {
    if (!asked || event.isComposing) return;
    const count = asked.question.options.length;
    const keys = {
      Enter: () => answer("yes"),
      Escape: () => answer("no"),
      ArrowDown: () => count && ((asked.current = (asked.current + 1) % count), drawQuestion()),
      ArrowUp: () => count && ((asked.current = (asked.current + count - 1) % count), drawQuestion()),
      " ": () => asked.question.multiple && tick(),
    };
    keys.ArrowRight = keys.ArrowDown;
    keys.ArrowLeft = keys.ArrowUp;
    const act = keys[event.key];
    if (!act) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    act();
  },
  { capture: true },
);

async function load() {
  const state = await invoke("chat_state");
  messages.replaceChildren();
  live = null;
  input.disabled = !state;
  input.placeholder = state
    ? "Powiedz, co dalej… (Enter wysyła, Esc zatrzymuje)"
    : "Rozmowa zacznie się, gdy tablica się otworzy.";
  for (const entry of state?.entries ?? []) add(entry);
  setRunning(state?.running ?? false);
  engine.disabled = model.disabled = !state;
  if (state) {
    engine.value = state.engine;
    model.value = state.model;
    offerModels(state.engine);
  }
  toEnd();
}

// The models to suggest. For pi they come from pi itself, on the machine
// where it runs.
async function offerModels(name) {
  let list = [];
  try {
    list = await invoke("chat_models", { engine: name });
  } catch {}
  models.replaceChildren(...list.map((m) => Object.assign(document.createElement("option"), { value: m })));
  return list;
}

// A new engine or model applies to the next message. Lemat saves it in the
// project's settings; a new engine gets the history in a new session.
async function saveEngine() {
  try {
    await invoke("chat_set_engine", { engine: engine.value, model: model.value.trim() });
  } catch (error) {
    add({ kind: "error", text: String(error), work: {} });
  }
  input.focus();
}

engine.addEventListener("change", async () => {
  const list = await offerModels(engine.value);
  // A Claude alias means nothing to pi, and the other way round.
  if (!list.includes(model.value)) model.value = list[0] ?? "";
  if (model.value) await saveEngine();
  else model.focus();
});
model.addEventListener("change", () => {
  if (model.value.trim()) saveEngine();
});
model.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    model.blur();
  }
});

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
await listen("chat-question", (event) => showQuestion(event.payload));
window.addEventListener("board-path", load);
await load();
input.focus();
