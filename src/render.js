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
