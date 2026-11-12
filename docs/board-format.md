# The board format

A board is a plain Markdown file with LaTeX. Any editor and any agent can
read and change it. Two kinds of HTML comments mark the steps and their
checks. Markdown viewers do not show these comments, so the file still
reads well without Lemat.

## Example

```markdown
# Granica ciągu

Oblicz granicę:

$$\lim_{n\to\infty}\frac{3n^2-5n+1}{2n^2+7}$$

<!-- step 1 -->
**1.** Dzielę licznik i mianownik przez $n^2$.

$$
\frac{3-\frac{5}{n}+\frac{1}{n^2}}{2+\frac{7}{n^2}}
$$

<!-- mark checked -->
> ✓ Dzielenie jest poprawne.

<!-- step 2 -->
**2.** Przechodzę do granicy.

$$
\frac{2}{3}
$$

<!-- mark wrong -->
> ⚠ Licznik dąży do 3, a mianownik do 2, więc granica to 3/2.
```

## Parts

- **The head**: everything before the first step: the title, the task,
  notes. Lemat does not change it.
- **A step** starts at a line `<!-- step N -->` and ends at the next step
  marker or at the end of the file. `N` is a positive whole number. It
  names the step: the board tools find the step by it. The board tools
  write the step as `**N.**`, a short note in words, and the formula on
  its own lines between `$$`.
- **A mark** is a line `<!-- mark checked -->` or `<!-- mark wrong -->`
  inside a step, followed by one quoted line: `> ✓ note` or `> ⚠ note`.
  The note is one sentence. For a wrong step it says what is wrong; it
  does not fix the step. A step has at most one mark.

Each marker stands alone on its line. Formulas use `$...$` inline and
`$$...$$` on their own lines.

## Rules for the tools

- `board_step` without a number adds a step after the highest number.
  With the number of an existing step it replaces that step's text and
  removes its mark, because the new text needs a new check.
- `board_mark` replaces the step's mark.
- The tools keep text they do not know. A line written by hand inside a
  step stays in that step. The tools may move it above the step's mark.
- The tools rewrite the whole file. They normalize blank lines between
  the parts.

## How Lemat shows it

Lemat draws each step as a section with a line on its left side: green
for a checked step, amber for a wrong one, grey for a step without a
mark. The mark's note shows in a box under the step.
