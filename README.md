# Lemat

Lemat is a math board that works with a coding agent. You lead a proof or
a calculation one sentence at a time, and the agent writes each step in
LaTeX, checks it, and marks mistakes. The board shows the steps as they
change.

The agent is Claude Code, pi, or later Codex, through
[Lejce](https://github.com/KrzaQ/lejce). It can run on this machine or in
a folder on another machine over SSH, where your notes and course files
live. Lemat gives it MCP servers: its own (the board) and any you add.

Lemat works well with voice input, for example from
[Kuterlapa](https://github.com/KrzaQ/kuterlapa): dictate into Lemat's text
field like into any other window.

Status: early. `docs/design.md` describes the plan.

## Build

Needs Rust and Node.js.

```sh
make install  # build Lemat.app, put it in ~/Applications, link `lemat` in ~/.local/bin
make run      # development build with live reload
make build    # only build Lemat.app
```

`make help` lists all targets. `APP_DIR` and `BIN_DIR` change where
`make install` puts the app and the link. In Finder, Open With → Lemat
opens a board (`.md`) or a project file (`.toml`).

## Use

Open a board file with `lemat path/to/board.md`, or with Plik → Otwórz
(File → Open, ⌘O). With `make run`, pass the file as
`make run BOARD=board.md`. Lemat opens the last file again on the next
start. It redraws the board within a second when the file changes, for
example after an edit in another editor.

The panel on the right is the conversation with the agent. Type or
dictate one sentence into the text field and press Enter. The agent
writes the step on the board and checks it. Esc or ⌘. stops it. Its work
(thinking, tool calls) stays folded above each answer; click to open it.

When the agent wants to run a command, or an engine extension asks
something, a dialog shows the question. Return answers yes (or takes
the option), Escape answers no, the arrows move between options. A game
pad through Kuterlapa works the same way. Lemat's own board tools need
no question. With `permissions = "auto"` in the settings, Claude Code's
classifier decides instead, and the agent gets no shell.

Under the text field you choose the engine and the model for the next
message. Lemat saves them in the settings file. A new engine gets the
conversation so far in a new session. Each answer says who gave it.

Settings live in `lemat.toml` next to the board file. Without the file,
Lemat runs Claude Code (sonnet) on this machine, in the board's folder:

```toml
engine = "claude"          # claude or pi
model = "sonnet"
effort = "medium"          # optional
host = "server"            # an ssh destination; leave out for this machine
folder = "~/notes/math"    # the agent's folder on that machine
```

### A project on another machine

To keep the boards on a server, next to your notes there, open a project
file instead of a board: any `.toml` file with the settings above and a
`board` key. Keep it anywhere on this machine, for example
`~/lemat/analysis.toml`:

```toml
engine = "claude"
model = "opus"
host = "server"            # an ssh destination
folder = "~/notes/math"    # the project folder there
board = "limit.md"         # the board, inside the folder
```

Lemat then runs the agent in that folder over SSH, keeps the board and
the history there, and forwards its MCP server to the agent with
`ssh -R`. It reads the board over one shared SSH connection
(ControlMaster) twice a second, so an edit there shows up within a
second. SSH must log in without a password prompt (a key or an agent).
When it cannot connect, or the folder is missing, a note at the top of
the board says so, and Lemat tries again every five seconds.

Lemat names the agent's session explicitly, so a Claude Code in a
terminal in the same folder never gets its conversation. Lemat also
turns off your own Claude Code hooks in its sessions, so that a hook
that binds every session in a folder (a chat bridge, say) catches only
your terminal session. Set `hooks = true` to keep them.

The project folder can be the same folder where you work in a terminal.
To keep Lemat's boards apart from your notes, put them in a subfolder:
`board = "lemat/limit.md"`. The history goes next to the board.

The agent changes the board only through Lemat's own MCP tools:
`board_step` writes a step, `board_mark` marks it checked (✓) or wrong
(⚠, with one sentence about the mistake), `board_read` reads the board,
`board_open` opens another board or starts a new task. The board stays
plain Markdown; `docs/board-format.md` describes how it marks the steps.
Lemat serves the tools over HTTP on localhost, on a free port.

Lemat keeps the conversation in `lemat.jsonl` in the board's folder, on
the machine where the board is. All
boards in one folder share it. When the engine has lost its session, or
you change the engine, Lemat sends this history to a new session.

Clone with `--recurse-submodules`: Lejce, which runs the agents, is a git
submodule. `make` fetches it if the folder is empty.

## License

MIT or Apache 2.0, at your choice. KaTeX and marked (in `src/vendor/`)
keep their own MIT licenses.

## Credits

Written by krzaq together with Claude (Anthropic), through Claude Code.
