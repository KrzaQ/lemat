# Design

## What Lemat is

A desktop program (Tauri: Rust and a web view) with two parts in one
window:

- **the board**: a Markdown document with LaTeX, drawn with KaTeX;
- **the conversation** with the agent: what you said, what the agent
  answered, and its work (thinking, tool calls) folded.

You give the agent one sentence at a time: "divide by n squared", "now go
to the limit". The agent turns each sentence into a numbered step on the
board and checks it. A wrong step gets a warning with one sentence about
the mistake. The agent does not fix your step unless you ask: you lead the
proof.

## The agent

Lemat runs the agent through Lejce:

- engine: Claude Code, pi, later Codex;
- host: this machine, or a folder on another machine over SSH;
- one engine process per conversation, kept alive between messages;
- an explicit session id, so that another agent in the same folder (for
  example a Claude Code in a terminal) never gets mixed in.

Two agents in one folder share files, `CLAUDE.md`, memory and the
account's usage limits. That is the point: a terminal session and Lemat
work on the same notes. Claude Code's Edit refuses to change a file that
changed since it was read, so two agents do not overwrite each other's
edits silently.

## The board's MCP server

Lemat serves MCP over HTTP on localhost. For an agent on another machine,
`ssh -R` forwards a port there to it. Tools:

- `board_open`: open a task (a new one, or an existing file);
- `board_step`: add or replace a step;
- `board_mark`: mark a step as checked or wrong, with a note;
- `board_read`: the document as it is now.

The settings add other MCP servers: local ones (forwarded the same way)
and ones on the agent's machine.

## Questions from the agent

Permission prompts and choices (Claude Code's `--permission-prompt-tool`,
pi's extension dialogs) appear in Lemat's window as a dialog, instead of
failing silently in a non-interactive run.

## History

Lemat owns the conversation history and the board documents, in its own
format, next to each other in the project folder on the agent's machine.
The engine's own session files only help to resume. When the engine
changes, or its session is gone, Lemat sends the history again.

## Settings

One file per project:

```toml
engine = "claude"          # claude, pi, codex
model = "opus"
host = "server"            # an ssh destination; leave out for this machine
folder = "~/notes/math"    # the project folder on that machine

[[mcp]]
name = "example"
url = "http://localhost:8123/mcp"
forward = true             # forward it to the agent's machine with ssh -R
```

## Steps

1. Done: the board shows a Markdown file and redraws it on change.
2. Done: the conversation through Lejce, with a local engine.
3. Done: the board's MCP server and its tools (`docs/board-format.md`).
4. Done: SSH hosts with forwarded MCP servers, the board and history there.
5. Done: questions from the agent in a dialog.
6. Done: history, and switching engines in a conversation.
