# Task Manager CLI

Terminal-first personal operations workspace written in Rust.

This app has grown past a simple task tracker. It now provides a launcher-driven TUI with:

- `Task Manager` for topic-based task tracking
- `Notes` for DB notes plus file-backed markdown notes
- `1:1 Manager` for leadership syncs, follow-ups, and action extraction
- `Delegation Tracker` for delegated work, reminders, and due dates
- `Decision Log` for decisions, rationale, reviews, and linked records

## What It Does

### Homepage

- launcher for all tools
- dashboard snapshots for tasks, notes, and leadership records
- recent activity and lightweight workspace context

### Task Manager

- topic-based task organization
- favourites and completed/special-task views
- filter presets and command palette
- focused handoff into linked task records

### Notes

- two note systems in one tool:
  - SQLite-backed notes
  - file-backed markdown notes under a notes root
- file browser with create, rename, move, copy, delete
- markdown preview and inline editor
- related links and backlinks
- daily-note and template support
- autosave for existing note edits and inline file edits

### Leadership Tools

- `1:1 Manager`
  - direct, skip-level, upward, and peer sync tracking
  - cadence, purpose, team/org, manager chain, action items
- `Delegation Tracker`
  - owners, status, due dates, follow-up dates, reminders
- `Decision Log`
  - decision status, rationale, impact, tags, review dates
  - linked notes and linked tasks

## Stack

- Rust 2021
- `tui` + `crossterm`
- SQLite via Diesel and embedded migrations
- `serde` / JSON for presets and lightweight tool state
- `slog` for terminal and file logging

## Run

```bash
cargo run
```

The app loads `.env` automatically if present.

## Configuration

Task Manager DB:

```env
TASK_MANAGER_DB_DIR=.task_manager
TASK_MANAGER_DB_FILENAME=task_manager.db
```

Notes DB and file root:

```env
NOTES_DB_DIR=.notes
NOTES_DB_FILENAME=notes.db
NOTES_ROOT_DIR=.notes/files
```

Useful runtime environment:

```env
RUST_LOG=info
```

Default generated data locations:

- tasks DB: `.task_manager/`
- notes DB: `.notes/`
- notes files: `.notes/files/`
- logs: `.logs/app.log`

## Controls

The app is intentionally keyboard-first. Common patterns:

- `Enter` opens or confirms
- `q` backs out of a tool or quits from the homepage
- `:` opens the command palette where supported
- `/` starts filtering/search where supported
- arrow keys always work for navigation

Some tools also support vim-style movement in navigation modes. Text-entry modes use typed characters normally.

## Development

Useful commands:

```bash
cargo check
cargo test
cargo fmt
```

## Tests

The project uses both inline unit tests and top-level integration tests.

- inline unit tests live under `src/...`
- integration tests live under [tests](/Users/kenneth.thomas/Workspace/task_manager_cli/tests)

Current coverage includes:

- DB lifecycle
- task-manager form and filter behavior
- notes file browser, templates, links, filters, and editor flows
- command palette behavior
- autosave and input-mode regressions

## Project Layout

High-level structure:

- [src/homepage.rs](/Users/kenneth.thomas/Workspace/task_manager_cli/src/homepage.rs) and `src/homepage/`
- [src/task_manager](/Users/kenneth.thomas/Workspace/task_manager_cli/src/task_manager)
- [src/notes](/Users/kenneth.thomas/Workspace/task_manager_cli/src/notes)
- [src/leadership_tools](/Users/kenneth.thomas/Workspace/task_manager_cli/src/leadership_tools)
- [src/common](/Users/kenneth.thomas/Workspace/task_manager_cli/src/common) for shared palette, TUI, logging, and popup utilities

## Current Status

The app is actively evolving, but it is no longer just an early scaffold.

Current baseline:

- launcher and tool switching are working
- local persistence is working
- test suite is in place and exercised regularly
- leadership and notes workflows are materially functional

Main current limitation:

- persistence is local-first and single-user; there is no sync or multi-user model
