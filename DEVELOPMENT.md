# DEVELOPMENT

## Purpose

This document defines how `tmuxr` should be developed: the engineering principles, the delivery approach, and the phased plan for turning a simple helper concept into a reliable Rust TUI application.

## Development Philosophy

`tmuxr` should be developed as an iterative product, not as a one-shot rewrite.

That means:

- start from the real workflows proven by `tmuxh`
- ship thin vertical slices
- validate usability early
- keep architecture modular enough to grow
- avoid premature complexity until the session-management core is stable

The most important rule is:

Build a small but real TUI that works end-to-end before chasing broad feature coverage.

## Product Development Method

The recommended methodology is a hybrid of:

- iterative incremental delivery
- CLI/TUI-first prototyping
- test-driven validation for core command logic
- usability-driven refinement for interaction design

In practice, this means each phase should produce something executable and reviewable.

## Guiding Principles

### 1. Preserve Practical Value

The first versions of `tmuxr` must cover the common tasks already handled by `tmuxh`. Existing usefulness should not be lost during the rewrite.

### 2. Separate UI from tmux Operations

The codebase should isolate:

- TUI rendering
- input/event handling
- tmux command execution
- application state and action routing

This separation will make the project easier to test, extend, and debug.

### 3. Optimize for Terminal Ergonomics

A TUI is not just a colored CLI. Interaction should be designed intentionally for:

- arrow keys
- Enter / Escape behavior
- tab focus movement
- mouse click support
- confirmation flows
- helpful empty and error states

### 4. Prefer Stable Cross-Platform Abstractions

Rust libraries and standard process invocation should be used wherever possible instead of shell-specific behavior. Platform-specific code should be introduced only when necessary and kept behind clear boundaries.

### 5. Keep tmux as the Source of Truth

`tmuxr` should orchestrate `tmux`, not duplicate its runtime model. The app should query `tmux` state, transform it into structured application data, and execute user actions through explicit `tmux` commands.

## Recommended Technical Architecture

The initial architecture should be split into modules with clear ownership.

### `domain`

Structured models for:

- session
- window
- pane
- client context
- action/result/error types

### `tmux`

Integration layer responsible for:

- detecting whether `tmux` is available
- detecting whether the app runs inside an attached client
- executing `tmux` commands
- parsing command output into typed models
- mapping failures into friendly domain errors

### `app`

Application state and orchestration:

- current screen
- focused element
- selected item
- modal/dialog state
- action dispatch
- refresh logic

### `ui`

TUI rendering and user interaction:

- layout
- list widgets
- detail panels
- status bar
- help dialogs
- confirmation modals

### `config`

Optional later-stage support for:

- key bindings
- theme
- startup behavior
- mouse enable/disable

## Recommended Tooling

- Language: Rust stable
- Build tool: Cargo
- TUI: `ratatui`
- Terminal backend and events: `crossterm`
- Error handling: `thiserror` or `anyhow` depending on layer
- Serialization for future config: `serde`
- Testing:
  - unit tests for parsing and action logic
  - integration tests for command wrappers where feasible
  - manual UX testing in real terminals

## Delivery Strategy

Development should proceed in milestones. Each milestone should end with a runnable artifact and a short review.

### Milestone 0: Project Foundation

Outcome:

- Rust project scaffold
- dependency selection
- module layout
- basic coding conventions

### Milestone 1: Session-Centric MVP

Outcome:

- full-screen TUI
- session list
- keyboard navigation
- create / attach / detach / kill session flows
- help screen

This milestone is the first release candidate for replacing `tmuxh` in daily use.

### Milestone 2: UX Maturity

Outcome:

- mouse support
- confirmation dialogs
- status/error messaging
- refresh behavior
- more polished layout

### Milestone 3: Window and Pane Operations

Outcome:

- window list and actions
- pane list and actions
- split, move, rename, kill flows

### Milestone 4: Power-User Features

Outcome:

- filtering/search
- command palette
- configurable shortcuts
- optional persistent config

## Development Workflow

The team should use the following loop:

1. Define one small user-visible capability.
2. Identify the `tmux` commands required for that capability.
3. Implement the domain and command layer first.
4. Add the TUI flow on top.
5. Test both inside and outside `tmux`.
6. Review UX friction before adding more features.

This prevents the UI from getting ahead of the real execution model.

## Testing Strategy

### Unit Tests

Use unit tests for:

- parsing `tmux list-*` outputs
- state transitions
- selection logic
- dialog behavior
- error mapping

### Integration Tests

Use integration tests where practical for:

- command builder behavior
- end-to-end action wiring
- environment detection

Because `tmux` interactions depend on the host environment, some tests may need controlled local fixtures or selective execution.

### Manual Validation Matrix

Every user-facing milestone should be checked in at least these scenarios:

- outside `tmux`
- inside `tmux`
- terminal with mouse reporting enabled
- terminal without mouse interaction
- no active sessions
- one active session
- multiple sessions
- invalid or disappearing session targets

## UX Review Criteria

A feature is not done just because it works technically. It should also satisfy:

- Can a new user discover the action?
- Can the user recover from mistakes?
- Is destructive behavior confirmed?
- Is the current focus obvious?
- Is the screen still usable in a smaller terminal?
- Does the interface remain fast with multiple sessions/windows?

## Error Handling Policy

Errors should be:

- specific enough for debugging
- friendly enough for routine usage
- surfaced in the UI without crashing the app unless the error is fatal

Typical examples:

- `tmux` not installed
- server not running
- target session no longer exists
- command failed due to context mismatch

## Compatibility Policy

The target should be broad OS compatibility at the application level, but actual runtime support still depends on `tmux` availability on the host.

Practical target environments:

- Linux
- macOS
- Windows environments that can run `tmux` through compatible shells or subsystems

The app should avoid promising more than the underlying `tmux` environment can support.

## Documentation Policy

Documentation should evolve with the codebase.

At minimum, each major phase should keep these updated:

- `README.md`: product purpose and usage direction
- `DEVELOPMENT.md`: method and architecture decisions
- `TODO.md`: current actionable task list

## Decision-Making Heuristics

When choosing between two implementations, prefer the one that:

- improves portability
- reduces shell-specific assumptions
- keeps core logic testable
- produces a clearer terminal interaction model
- does not block incremental delivery

## First Implementation Priority

If there is a conflict between breadth and completion, prefer:

- fewer features
- better navigation
- clearer state handling
- stronger session-management reliability

That is the fastest path to a useful `tmuxr`.
