// Draws Markdown with LaTeX into an element: marked for the Markdown, then
// KaTeX for the formulas.

import { toBoardHtml, toHtml } from "./render.js";

export function drawMarkdown(element, text) {
  element.innerHTML = toHtml(text, (markdown) => marked.parse(markdown));
  drawFormulas(element);
}

// The board: Markdown with its steps and their marks.
export function drawBoard(element, text) {
  element.innerHTML = toBoardHtml(text, (markdown) => marked.parse(markdown));
  drawFormulas(element);
}

function drawFormulas(element) {
  renderMathInElement(element, {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
    ],
    throwOnError: false,
  });
}
