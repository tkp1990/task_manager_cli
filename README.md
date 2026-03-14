# task_manager_cli

Terminal-based personal productivity app written in Rust. It currently includes two tools behind a simple launcher:

- `Task Manager`: topic-based task tracking with favourites, completion state, and log output
- `Notes`: lightweight note creation, editing, viewing, and deletion

## Stack

- Rust 2021
- `tui` + `crossterm` for the terminal UI
- SQLite with Diesel and embedded migrations
- `slog` for terminal and JSON file logging

## Running

1. Ensure Rust is installed.
2. From the project root, run:

```bash
cargo run
```

The app reads `.env` at startup. Current task-manager settings are:

```env
TASK_MANAGER_DB_DIR=.task_manager
TASK_MANAGER_DB_FILENAME=task_manager.db
```

The notes app falls back to these defaults if not set:

```env
NOTES_DB_DIR=.notes
NOTES_DB_FILENAME=notes.db
```

Logs are written to `.logs/app.log`.

## Controls

### Homepage

- `Up` / `Down`: move between tools
- `Enter`: launch selected tool
- `q`: quit

### Task Manager

- `j` / `k` or arrow keys: move through tasks
- `h` / `l` or arrow keys: switch topics
- `a`: add task
- `e`: edit selected task description
- `d`: delete selected task
- `t`: toggle completion
- `f`: toggle favourite
- `N`: add topic
- `X`: delete current topic when allowed
- `W`: open favourites/completed popup
- `H`: toggle help
- `q`: return to homepage

### Notes

- `j` / `k` or arrow keys: move through notes
- `a`: add note
- `e`: edit selected note
- `d`: delete selected note
- `Enter`: view selected note
- `H`: open help
- `q`: return to homepage

## Current State

The repo is functional but still early-stage:

- the core TUI flows compile and run
- persistence is local-only through SQLite files in the repo directory
- there is no automated test coverage yet
- the project is still evolving, especially around the notes feature

## Cleanup Baseline

Useful commands while working on the repo:

```bash
cargo fmt
cargo test
```
