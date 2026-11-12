# Lemat — notes for Claude

Lemat is a math board that works with a coding agent (Tauri: Rust in
`src-tauri/`, the web view in `src/`). `docs/design.md` holds the plan.
Lejce (a separate repository) runs the agents.
Expect the author's messages to come from dictation, with words misheard.

## The repository is public

- No private data in files, file names or commit messages: no host names,
  internal domains, IP addresses, home paths, people's names or personal
  settings. Use generic examples ("server", "~/notes").
- `make hooks` installs a pre-commit hook (`scripts/check-private.sh`). It
  checks staged changes against private patterns kept outside the
  repository. Do not bypass it with `--no-verify`.

## Build and run

- `make install`, `make run`, `make build`, `make test`. `make help` lists
  all targets.
- Third-party code for the web view lives in `src/vendor/`, so that the
  app works offline. No CDN.

## Conventions

- Code, comments and documentation in English. User-facing text may be
  Polish.
- Comments explain why, not what.
- Commits:
  - subject in imperative mood, under about 70 characters, with a module
    prefix: `app:` (Rust), `ui:` (web view), `build:`, `docs:`;
  - the body explains why;
  - one commit per module or concern;
  - no AI attribution in commits (no `Co-Authored-By`, no tool names).
    Credit lives in `README.md`.
