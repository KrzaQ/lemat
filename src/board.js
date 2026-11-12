// The board: a Markdown document with LaTeX, drawn with marked and KaTeX.
// For now it shows a sample; later the Rust side sends the document of the
// open task and every change to it.

const SAMPLE = String.raw`# Lemat

Oblicz granicę:

$$\lim_{n\to\infty}\frac{3n^2-5n+1}{2n^2+7}$$

1. Dzielę licznik i mianownik przez $n^2$:
   $$\frac{3-\frac{5}{n}+\frac{1}{n^2}}{2+\frac{7}{n^2}}$$
2. $\frac{1}{n} \to 0$ oraz $\frac{1}{n^2} \to 0$, więc granica to $\frac{3}{2}$.
`;

// marked would take "\\" and "_" inside formulas for Markdown, so formulas
// go out before marked and come back after it.
function render(markdown) {
  const formulas = [];
  const kept = markdown.replace(/\$\$[\s\S]+?\$\$|\$[^$\n]+?\$/g, (formula) => {
    formulas.push(formula);
    return `@@formula${formulas.length - 1}@@`;
  });
  const html = marked.parse(kept).replace(/@@formula(\d+)@@/g, (_, i) =>
    formulas[i].replace(/&/g, "&amp;").replace(/</g, "&lt;"),
  );
  const board = document.getElementById("board");
  board.innerHTML = html;
  renderMathInElement(board, {
    delimiters: [
      { left: "$$", right: "$$", display: true },
      { left: "$", right: "$", display: false },
    ],
    throwOnError: false,
  });
}

render(SAMPLE);
