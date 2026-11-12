//! The board document: plain Markdown with LaTeX, where small HTML
//! comments mark the steps and their checks. `docs/board-format.md`
//! describes the format.
//!
//! ```markdown
//! # Granica ciągu
//!
//! $$\lim_{n\to\infty}\frac{3n^2-5n+1}{2n^2+7}$$
//!
//! <!-- step 1 -->
//! **1.** Dzielę licznik i mianownik przez $n^2$.
//!
//! $$\frac{3-\frac{5}{n}+\frac{1}{n^2}}{2+\frac{7}{n^2}}$$
//!
//! <!-- mark checked -->
//! > ✓ Dzielenie jest poprawne.
//! ```
//!
//! Text that the parser does not know stays as it is, so hand edits and
//! other tools survive.

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Checked,
    Wrong,
}

impl State {
    pub fn parse(name: &str) -> Option<State> {
        match name {
            "checked" => Some(State::Checked),
            "wrong" => Some(State::Wrong),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            State::Checked => "checked",
            State::Wrong => "wrong",
        }
    }

    fn sign(self) -> &'static str {
        match self {
            State::Checked => "✓",
            State::Wrong => "⚠",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub state: State,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub number: u32,
    /// The step's Markdown, without its marker and its mark.
    pub body: String,
    pub mark: Option<Mark>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Document {
    /// Everything before the first step: the title and the task.
    pub head: String,
    pub steps: Vec<Step>,
}

fn step_marker(line: &str) -> Option<u32> {
    line.trim()
        .strip_prefix("<!-- step ")?
        .strip_suffix(" -->")?
        .trim()
        .parse()
        .ok()
}

fn mark_marker(line: &str) -> Option<State> {
    State::parse(
        line.trim()
            .strip_prefix("<!-- mark ")?
            .strip_suffix(" -->")?
            .trim(),
    )
}

impl Document {
    pub fn parse(text: &str) -> Document {
        let mut document = Document::default();
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if let Some(number) = step_marker(line) {
                document.steps.push(Step {
                    number,
                    body: String::new(),
                    mark: None,
                });
                continue;
            }
            let Some(step) = document.steps.last_mut() else {
                document.head.push_str(line);
                document.head.push('\n');
                continue;
            };
            if let Some(state) = mark_marker(line) {
                // The note is the quote right under the marker.
                let mut note = Vec::new();
                while let Some(quoted) = lines.peek().and_then(|l| l.trim().strip_prefix('>')) {
                    note.push(quoted.trim().to_string());
                    lines.next();
                }
                let note = note.join(" ");
                let note = note
                    .strip_prefix(state.sign())
                    .unwrap_or(&note)
                    .trim()
                    .to_string();
                step.mark = Some(Mark { state, note });
                continue;
            }
            step.body.push_str(line);
            step.body.push('\n');
        }
        document
    }

    pub fn render(&self) -> String {
        let mut text = self.head.trim_end().to_string();
        for step in &self.steps {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            let _ = write!(text, "<!-- step {} -->\n{}", step.number, step.body.trim());
            if let Some(mark) = &step.mark {
                let _ = write!(
                    text,
                    "\n\n<!-- mark {} -->\n> {} {}",
                    mark.state.name(),
                    mark.state.sign(),
                    one_line(&mark.note)
                );
            }
        }
        text.push('\n');
        text
    }

    fn find(&mut self, number: u32) -> Option<&mut Step> {
        self.steps.iter_mut().find(|s| s.number == number)
    }

    pub fn next_number(&self) -> u32 {
        self.steps.iter().map(|s| s.number).max().unwrap_or(0) + 1
    }

    /// Adds a step, or replaces the step with this number. A replaced step
    /// loses its mark: it needs a new check. Returns the step's number.
    pub fn set_step(&mut self, number: Option<u32>, note: &str, latex: Option<&str>) -> u32 {
        let number = number.unwrap_or_else(|| self.next_number());
        let mut body = format!("**{number}.** {}", note.trim());
        if let Some(latex) = latex.map(str::trim).filter(|l| !l.is_empty()) {
            let latex = latex
                .strip_prefix("$$")
                .and_then(|l| l.strip_suffix("$$"))
                .unwrap_or(latex)
                .trim();
            let _ = write!(body, "\n\n$$\n{latex}\n$$");
        }
        match self.find(number) {
            Some(step) => {
                step.body = body;
                step.mark = None;
            }
            None => {
                self.steps.push(Step {
                    number,
                    body,
                    mark: None,
                });
                self.steps.sort_by_key(|s| s.number);
            }
        }
        number
    }

    pub fn set_mark(&mut self, number: u32, state: State, note: &str) -> Result<(), String> {
        let step = self
            .find(number)
            .ok_or_else(|| format!("there is no step {number} on the board"))?;
        step.mark = Some(Mark {
            state,
            note: note.trim().to_string(),
        });
        Ok(())
    }
}

/// A mark's note is one quoted line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A new board: the title and the task.
pub fn new_board(title: &str, task: &str) -> String {
    format!("# {}\n\n{}\n", one_line(title), task.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = "# Granica\n\n$$\\lim_{n\\to\\infty} a_n$$\n";

    #[test]
    fn plain_markdown_is_all_head() {
        let document = Document::parse(BOARD);
        assert_eq!(document.head, BOARD);
        assert!(document.steps.is_empty());
        assert_eq!(document.render(), BOARD);
    }

    #[test]
    fn steps_and_marks_round_trip() {
        let mut document = Document::parse(BOARD);
        assert_eq!(
            document.set_step(None, "Dzielę przez $n^2$.", Some("\\frac{3}{2}")),
            1
        );
        assert_eq!(document.set_step(None, "Granica to $\\frac23$.", None), 2);
        document.set_mark(1, State::Checked, "Dobrze.").unwrap();
        document
            .set_mark(2, State::Wrong, "Licznik dąży do 3,\nmianownik do 2.")
            .unwrap();
        let text = document.render();
        assert_eq!(
            text,
            "# Granica\n\n$$\\lim_{n\\to\\infty} a_n$$\n\n\
             <!-- step 1 -->\n**1.** Dzielę przez $n^2$.\n\n$$\n\\frac{3}{2}\n$$\n\n\
             <!-- mark checked -->\n> ✓ Dobrze.\n\n\
             <!-- step 2 -->\n**2.** Granica to $\\frac23$.\n\n\
             <!-- mark wrong -->\n> ⚠ Licznik dąży do 3, mianownik do 2.\n"
        );
        let again = Document::parse(&text);
        assert_eq!(again.render(), text);
        assert_eq!(
            again.steps[1].mark,
            Some(Mark {
                state: State::Wrong,
                note: "Licznik dąży do 3, mianownik do 2.".into()
            })
        );
    }

    #[test]
    fn replacing_a_step_drops_its_mark() {
        let mut document = Document::default();
        document.set_step(None, "a", None);
        document.set_mark(1, State::Wrong, "źle").unwrap();
        document.set_step(Some(1), "b", Some("$$x$$"));
        assert_eq!(document.steps[0].mark, None);
        assert_eq!(document.steps[0].body, "**1.** b\n\n$$\nx\n$$");
    }

    #[test]
    fn hand_edits_survive() {
        let text = "# T\n\n<!-- step 1 -->\n**1.** a\n\nA line by hand.\n\n\
                    <!-- mark checked -->\n> ✓ ok\n\nMore by hand.\n";
        let mut document = Document::parse(text);
        assert!(document.steps[0].body.contains("A line by hand."));
        assert!(document.steps[0].body.contains("More by hand."));
        document.set_step(None, "b", None);
        let text = document.render();
        assert!(text.contains("A line by hand.") && text.contains("More by hand."));
    }

    #[test]
    fn mark_needs_an_existing_step() {
        let mut document = Document::default();
        assert!(document.set_mark(3, State::Checked, "ok").is_err());
    }

    #[test]
    fn steps_stay_in_order_and_number_after_the_last() {
        let mut document = Document::default();
        document.set_step(Some(5), "e", None);
        document.set_step(Some(2), "b", None);
        assert_eq!(document.set_step(None, "f", None), 6);
        let numbers: Vec<u32> = document.steps.iter().map(|s| s.number).collect();
        assert_eq!(numbers, [2, 5, 6]);
    }

    #[test]
    fn new_board_has_title_and_task() {
        assert_eq!(
            new_board("Granica", " Oblicz $x$. "),
            "# Granica\n\nOblicz $x$.\n"
        );
    }
}
