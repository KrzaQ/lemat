// Draws Markdown with LaTeX into an element: marked for the Markdown, then
// KaTeX for the formulas.

import { toHtml } from "./render.js";

export function drawMarkdown(element, text) {
  element.innerHTML = toHtml(text, (markdown) => marked.parse(markdown));
  renderMathInElement(element, {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
    ],
    throwOnError: false,
  });
}
