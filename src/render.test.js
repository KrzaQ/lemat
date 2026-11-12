// Run with `make test`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

import { takeFormulas, putFormulas, toHtml } from "./render.js";

// The vendored marked is a browser script; run it the way the page does.
function loadMarked() {
  const context = {};
  context.globalThis = context;
  vm.runInNewContext(
    readFileSync(new URL("vendor/marked.min.js", import.meta.url), "utf8"),
    context,
  );
  return context.marked;
}

const marked = loadMarked();
const parse = (text) => marked.parse(text);

test("formulas go out and come back unchanged", () => {
  const source = String.raw`a $x_1$ b $$\frac{1}{n}\\ y_2$$ c`;
  const { text, formulas } = takeFormulas(source);
  assert.equal(text, "a @@formula0@@ b @@formula1@@ c");
  assert.deepEqual(formulas, ["$x_1$", String.raw`$$\frac{1}{n}\\ y_2$$`]);
  assert.equal(putFormulas(text, formulas), source);
});

test("marked does not touch underscores and backslashes in formulas", () => {
  const html = toHtml(String.raw`Niech $a_n = b_n$ oraz $$x \\ y$$`, parse);
  assert.match(html, /\$a_n = b_n\$/);
  assert.match(html, /\$\$x \\\\ y\$\$/);
  assert.doesNotMatch(html, /<em>/);
});

test("Markdown around formulas still works", () => {
  const html = toHtml("# Tytuł\n\n1. krok $x^2$\n2. **drugi**", parse);
  assert.match(html, /<h1>Tytuł<\/h1>/);
  assert.match(html, /<li>krok \$x\^2\$<\/li>/);
  assert.match(html, /<strong>drugi<\/strong>/);
});

test("a display formula over several lines stays whole", () => {
  const html = toHtml("$$\n\\lim_{n\\to\\infty}\n\\frac{1}{n}\n$$", parse);
  assert.match(html, /\$\$\n\\lim_\{n\\to\\infty\}\n\\frac\{1\}\{n\}\n\$\$/);
});

test("< and & inside a formula cannot become HTML", () => {
  const html = toHtml("$a < b & c$", parse);
  assert.match(html, /\$a &lt; b &amp; c\$/);
});
