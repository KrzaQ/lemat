// Markdown with LaTeX to HTML. Pure functions, so that `node --test` can
// check them without a window.

const FORMULA = /\$\$[\s\S]+?\$\$|\$[^$\n]+?\$/g;
const PLACEHOLDER = /@@formula(\d+)@@/g;

// marked would take "\\" and "_" inside formulas for Markdown, so formulas
// go out before marked and come back after it.
export function takeFormulas(markdown) {
  const formulas = [];
  const text = markdown.replace(FORMULA, (formula) => {
    formulas.push(formula);
    return `@@formula${formulas.length - 1}@@`;
  });
  return { text, formulas };
}

export function putFormulas(html, formulas) {
  return html.replace(PLACEHOLDER, (_, i) =>
    formulas[i].replace(/&/g, "&amp;").replace(/</g, "&lt;"),
  );
}

// `parse` is marked.parse. KaTeX then finds the formulas in the HTML.
export function toHtml(markdown, parse) {
  const { text, formulas } = takeFormulas(markdown);
  return putFormulas(parse(text), formulas);
}

const STEP = /^\s*<!-- step (\d+) -->\s*$/;
const MARK = /^\s*<!-- mark (checked|wrong) -->\s*$/;

// Splits the board into its head and its steps, by the markers that
// docs/board-format.md describes. Each step knows its mark.
export function splitSteps(markdown) {
  const head = [];
  const steps = [];
  for (const line of markdown.split("\n")) {
    const step = line.match(STEP);
    if (step) {
      steps.push({ number: Number(step[1]), lines: [], mark: null });
      continue;
    }
    const current = steps.at(-1);
    if (!current) {
      head.push(line);
      continue;
    }
    const mark = line.match(MARK);
    if (mark) current.mark = mark[1];
    else current.lines.push(line);
  }
  return {
    head: head.join("\n"),
    steps: steps.map(({ number, lines, mark }) => ({ number, mark, text: lines.join("\n") })),
  };
}

// The board as HTML: each step in its own section, with the class of its
// mark, so that the style can show a check or a warning.
export function toBoardHtml(markdown, parse) {
  const { head, steps } = splitSteps(markdown);
  let html = toHtml(head, parse);
  for (const step of steps) {
    const mark = step.mark ? ` ${step.mark}` : "";
    html +=
      `<section class="board-step${mark}" data-step="${step.number}">` +
      toHtml(step.text, parse) +
      "</section>";
  }
  return html;
}
