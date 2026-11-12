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
make install
make run      # development build with live reload
make build    # release bundle
```

`make help` lists all targets.

## License

MIT or Apache 2.0, at your choice. KaTeX and marked (in `src/vendor/`)
keep their own MIT licenses.

## Credits

Written by krzaq together with Claude (Anthropic), through Claude Code.
