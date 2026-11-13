// Run with `make test`.

import { test } from "node:test";
import assert from "node:assert/strict";

import { metaLine, plural, seconds, tokens, workSummary } from "./words.js";

const tool = (n) => plural(n, "narzędzie", "narzędzia", "narzędzi");

test("Polish plural forms", () => {
  assert.deepEqual(
    [1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 112].map(tool),
    [
      "narzędzie", "narzędzia", "narzędzia", "narzędzi", "narzędzi", "narzędzi",
      "narzędzi", "narzędzi", "narzędzia", "narzędzi", "narzędzi",
    ],
  );
});

test("times and tokens stay short", () => {
  assert.equal(seconds(4.4), "4 s");
  assert.equal(seconds(75), "1 min 15 s");
  assert.equal(tokens(950), "950");
  assert.equal(tokens(41000), "41k");
  assert.equal(tokens(1234), "1.2k");
});

test("the meta line names who answered", () => {
  const work = { steps: [{}], seconds: 5 };
  assert.equal(metaLine("claude · opus", work), "claude · opus · 1 narzędzie · 5 s");
  assert.equal(metaLine(undefined, work), "1 narzędzie · 5 s");
  assert.equal(metaLine("pi · local/qwen", null), "pi · local/qwen");
});

test("work summary", () => {
  const work = { steps: [{}, {}, {}], seconds: 12.2, input: 41000, output: 1200 };
  assert.equal(workSummary(work), "3 narzędzia · 12 s · 41k → 1.2k");
  assert.equal(workSummary({ thinking: "hm", seconds: 2 }), "myślenie · 2 s");
  assert.equal(workSummary({}, true), "Pracuje…");
});
