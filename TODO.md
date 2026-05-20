# TODO

This document organizes the work for `tmuxr` by phase so development can move from concept to a usable Rust TUI application with clear priorities.

## Phase 0. Define and Scaffold

- [x] Create a new Rust project with Cargo.
- [x] Decide the initial crate type and directory structure.
- [x] Add baseline dependencies such as `ratatui`, `crossterm`, and error handling crates.
- [x] Define coding conventions, naming rules, and module boundaries.
- [x] Add a `.gitignore` appropriate for Rust development if needed.
- [x] Initialize a simple application entry point that can launch and exit cleanly.

## Phase 1. Core tmux Integration

- [x] Detect whether the `tmux` binary is installed.
- [x] Detect whether the app is running inside an active `tmux` client.
- [x] Implement a command runner wrapper around `tmux`.
- [x] Implement typed parsing for `tmux list-sessions`.
- [x] Define domain models for session metadata.
- [x] Add friendly error types and messages for common failures.
- [x] Verify behavior when no tmux server or no sessions exist.

## Phase 2. Minimal TUI Framework

- [x] Set up terminal initialization and cleanup.
- [x] Enter alternate screen mode and restore terminal state on exit.
- [x] Enable keyboard event handling.
- [x] Add a base layout with header, content area, footer, and status line.
- [x] Implement a simple screen/state model.
- [x] Add an event loop with render and input handling.
- [x] Ensure the app handles resize events gracefully.

## Phase 3. Session List MVP

- [x] Render sessions in a selectable list.
- [x] Support up/down cursor navigation.
- [x] Support Enter to select the highlighted action target.
- [x] Show session metadata such as name, window count, and attach state.
- [x] Display an empty state when no sessions exist.
- [x] Add refresh support to reload session state.
- [x] Make the selected item visually obvious.

## Phase 4. Session Actions

- [x] Implement create session flow.
- [x] Implement create detached session flow.
- [x] Implement attach or switch flow depending on current tmux context.
- [x] Implement detach current client flow.
- [x] Implement kill session flow.
- [x] Add confirmation dialog for kill actions.
- [x] Surface success and failure messages in the UI.

## Phase 5. Help and Guidance

- [x] Add an in-app help screen.
- [x] Add a built-in hotkey guide.
- [x] Add contextual hints in the footer or status bar.
- [x] Document primary keyboard shortcuts inside the UI.
- [x] Add clear cancel and back navigation behavior.

## Phase 6. Mouse Support and UX Polish

- Enable mouse capture where terminal support exists.
- Support clicking list items to focus/select them.
- Support clicking action buttons or dialog controls.
- Verify behavior when mouse support is unavailable.
- Improve focus styling and interaction feedback.
- Tune layouts for small and medium terminal sizes.

## Phase 7. Window Management

- Add `tmux list-windows` integration.
- Define window domain models.
- Build a window list/detail screen.
- Implement create window flow.
- Implement rename window flow.
- Implement switch window flow.
- Implement kill window flow with confirmation.

## Phase 8. Pane Management

- Add `tmux list-panes` integration.
- Define pane domain models.
- Build a pane list/detail screen.
- Implement horizontal and vertical split flows.
- Implement pane focus movement actions.
- Implement pane resize actions.
- Implement kill pane flow with confirmation.

## Phase 9. Robustness and Quality

- Add unit tests for tmux output parsing.
- Add unit tests for state transitions and selection logic.
- Add tests for error mapping and user feedback paths.
- Review cleanup behavior on panic or unexpected exit.
- Test repeatedly inside and outside tmux sessions.
- Test across Linux and macOS if available.

## Phase 10. Packaging and Adoption

- Add build instructions to the README.
- Define a versioning strategy.
- Prepare release build settings.
- Add installation instructions for common platforms.
- Collect feedback from real tmux users.
- Prioritize follow-up improvements based on observed usage.

## Backlog Ideas

- Search/filter sessions, windows, and panes.
- Command palette style quick actions.
- Configurable key bindings.
- Theme customization.
- Startup screen preferences.
- Session templates or presets.
- Integration with common project directories.

## Suggested Immediate Next Tasks

- Add mouse-based selection and action triggers.
- Improve focus styling and feedback for dialogs.
- Tune layouts for smaller terminals.
