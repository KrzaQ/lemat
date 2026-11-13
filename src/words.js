// Short Polish labels for the conversation. Pure, so that node can test
// them.

// Polish has three plural forms: 1 narzędzie, 2–4 narzędzia, 5 narzędzi
// (but 22 narzędzia, 12 narzędzi).
export function plural(n, one, few, many) {
  if (n === 1) return one;
  const tens = n % 100;
  const units = n % 10;
  if (units >= 2 && units <= 4 && (tens < 12 || tens > 14)) return few;
  return many;
}

export function seconds(s) {
  if (s < 60) return `${Math.round(s)} s`;
  return `${Math.floor(s / 60)} min ${Math.round(s % 60)} s`;
}

export function tokens(n) {
  return n >= 1000 ? `${(n / 1000).toFixed(1).replace(".0", "")}k` : `${n}`;
}

// The line above an answer: who answered, then the work.
export function metaLine(by, work) {
  return [by, work ? workSummary(work) : ""].filter(Boolean).join(" · ");
}

// One line about the agent's work: "3 narzędzia · 12 s · 41k → 1.2k".
export function workSummary(work, running = false) {
  const parts = [];
  const steps = work.steps?.length ?? 0;
  if (steps) parts.push(`${steps} ${plural(steps, "narzędzie", "narzędzia", "narzędzi")}`);
  else if (work.thinking) parts.push("myślenie");
  if (work.seconds != null) parts.push(seconds(work.seconds));
  if (work.input) parts.push(`${tokens(work.input)} → ${tokens(work.output ?? 0)}`);
  const text = parts.join(" · ");
  return running ? `Pracuje… ${text}`.trim() : text;
}
