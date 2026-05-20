# tmuxr

`tmuxr` is a Rust application that makes complex `tmux` operations easier to use through a terminal-based TUI menu interface.

The goal is simple:

- keep the user inside the terminal
- reduce the need to memorize `tmux` commands
- make common operations easier through menus, lists, dialogs, and shortcuts
- support both keyboard navigation and mouse interaction where the terminal allows it

## What tmuxr Is

`tmuxr` is a usability layer on top of `tmux`.

It does not try to replace `tmux`. Instead, it helps users access important `tmux` features through a more discoverable and comfortable terminal UI.

This is especially useful when:

- you know `tmux` is powerful, but its command set feels complex
- you use `tmux` often, but still want faster access to routine actions
- you work in text-only environments such as SSH sessions or remote servers
- you want a terminal-native workflow instead of a separate GUI

## Project Objective

Build a cross-platform Rust TUI application that lets users operate major `tmux` features in an easy, menu-driven way from the terminal.

The product should feel:

- easy to approach for users who do not remember many commands
- efficient for users who already know `tmux`
- safe for destructive actions through explicit confirmations
- extensible enough to grow into a full `tmux` companion

## Key Experience Goals

- Navigate with arrow keys, `Tab`, Enter, and Escape
- Select items from visible menus instead of typing many commands manually
- Use mouse click interaction where terminal support exists
- See clear status, help, and confirmation messages
- Stay usable both inside and outside an active `tmux` client

## Expected Feature Areas

### Session Management

- create sessions
- create detached sessions
- list sessions with metadata
- attach or switch to sessions
- rename sessions
- kill sessions

### Window Management

- list windows
- create windows
- rename windows
- switch windows
- kill windows

### Pane Management

- list panes
- split panes horizontally and vertically
- move focus between panes
- resize panes
- kill panes

### Navigation and Assistance

- keyboard-first navigation
- mouse input support
- built-in help and hotkey references
- confirmation dialogs for risky actions
- clear empty states and error feedback

## Design Principles

- Terminal-first: all primary workflows should work well in a text terminal.
- Discoverable: important actions should be visible rather than hidden behind memory-heavy commands.
- Safe: destructive operations should be confirmed.
- Portable: implementation should rely on Rust and terminal abstractions that work across major operating systems.
- Extensible: the codebase should support growth from session management into broader `tmux` control flows.

## Technical Direction

The application is being built in Rust with a TUI-focused architecture.

Planned foundational technologies:

- `ratatui` for interface rendering
- `crossterm` for terminal events and input handling
- `std::process::Command` for `tmux` command execution

This direction supports:

- cross-platform compatibility
- strong control over keyboard and mouse events
- maintainable structure compared with shell-centric implementations
- a clean path toward testing and iterative delivery

## Early Success Criteria

The first meaningful version of `tmuxr` should:

- run as a full-screen terminal UI
- show a selectable session list
- support cursor-key navigation
- support mouse selection where available
- create, attach, detach, and kill sessions
- behave correctly both inside and outside `tmux`

## Current Status

The repository is in the early build stage.

Phase 0 is focused on:

- Rust project scaffolding
- dependency setup
- module boundaries
- a clean application entry point

The next implementation focus after scaffolding is a read-only session list MVP.

See:

- [DEVELOPMENT.md](./DEVELOPMENT.md)
- [TODO.md](./TODO.md)
