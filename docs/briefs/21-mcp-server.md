# MCP Server (owner request, 2026-11-09)

**Goal.** Prism runs a local Model Context Protocol server, so an AI assistant such as Claude Code can read the model, the selection and Model Health, and run Prism's actions, each one undoable through the same command log (PLAN section 8, decision 6).

**Read first:** PLAN section 8 decision 6; `crates/prism-app/src/app.rs` (how actions reach the session); `actions/registry.rs`; the MCP spec, revisions 2025-11-25 and 2026-07-28 (Streamable HTTP, `server/discover`).

## Parts
M chunk, two parts (PLAN 6.0a).
1. **The server, reading, and actions** (this brief).
2. **Drawing and seeing:** tools that use Prism's tools at model coordinates (click points, type values in the measurement box), a viewport screenshot, replaying a recorded session, and more model detail (faces and edges).

## Requirements (part 1)
- **Off by default.** Help > MCP Server… opens a window: a switch to let AI assistants connect, the address, the token (with New Token), and the `claude mcp add` command to copy. It shows whether the server is listening (or why not, such as the port being in use) and the last few calls. The settings are kept in the user's settings, not in documents.
- **Transport:** Streamable HTTP on `http://127.0.0.1:<port>/mcp`, listening on the local machine only.
  - Every request needs `Authorization: Bearer <token>` (401 without it).
  - A request with an `Origin` header that is not a local address is refused (403), against DNS rebinding.
  - POST answers with JSON (no streams); a notification gets 202. GET and DELETE get 405. No sessions.
- **Both protocol eras:**
  - Older clients (2025-03-26 to 2025-11-25): `initialize` (the client's version if known, else 2025-11-25), `notifications/initialized`, `ping`.
  - 2026-07-28: no handshake; `server/discover`; the version in each request's `_meta` (and the `MCP-Protocol-Version` header, which must agree); an unknown version gets 400 with `UnsupportedProtocolVersion` (-32022) listing ours; an unknown method gets 404 with -32601. Results carry `resultType: "complete"`; `tools/list` carries `ttlMs` and `cacheScope`.
- **Tools (part 1):**
  - `get_model`: the document, units, tool, counts, the open group, the groups and components (Outliner rows), the view style, whether undo and redo can run.
  - `get_selection`: what Entity Info shows.
  - `get_health`: the Model Health rows.
  - `list_actions`: every action (id, name, menu path, key, whether it can run now and why not), filtered by an optional query.
  - `run_action`: run an action by id, exactly as a menu click does (greyed ones are refused with the reason). Undo and Redo are actions too.
- **Threads:** the server runs on its own thread. Tool calls go to the app's main thread through a channel and wake the event loop; the server waits for the answer (with a time limit).
- **Native only.** On the web build the window says it is not available (browsers cannot listen).

## Requirements (part 2)
- **`use_tool`:** pick a tool (optional), then run steps as a person would: `click` or `move` at a model point (metres, world coordinates; projected through the current camera, which first finishes any easing), `type` a value for the measurement box (then Enter), `key` (Enter, Escape, Backspace, arrows), `lock` an axis (red, green, blue: the arrow keys). Each click reports the point inference snapped to. A point outside the view stops the steps with a message to change the view first. It answers with the tool's hint, the measurement box, the counts and whether Undo can run. Everything goes through the session's events, so it is undoable and recorded like a person's input.
- **`get_geometry`:** the open group's faces (normal, area, outer loop points, number of holes) and edges (end points, length), in world coordinates and metres, up to a limit (default 200 each, with the totals).
- **`screenshot`:** the viewport as a PNG image (at most 1024 points wide), taken after the next frame is drawn, so it shows the latest change.
- **`replay_session`:** play a recorded session (Help > Record Session) from a file path or its JSON text, in a separate session (the open document is not touched), and say whether the end matches what was recorded, with the end's counts.
- **`get_model`** also gives the display units and the model's bounds.

## Tests (part 2)
- `use_tool` draws a rectangle on the ground by clicks and pushes it up by a typed value: the faces and their height are right; a point behind the camera stops with a message; Undo takes it back.
- `get_geometry` lists the cube's faces with unit normals and the right areas, and honours the limit.
- `replay_session` of a session recorded in the test matches; a changed result does not; a bad path is an error.
- The PNG encoder writes a valid PNG (signature, IHDR, the pixels back through inflate), base64 matches known values, and the shrink keeps the aspect.
- The protocol returns image content for an image result.

## Out of scope (part 1)
Part 2's items; resources and prompts; remote (non-local) connections; OAuth.

## Tests
- Protocol (pure): `initialize` for a known and an unknown version; `server/discover`; `tools/list`; `tools/call` to a fake host; an unknown tool; an unknown method (both eras); a notification; a 2026-07-28 request with an unknown version; a header mismatch.
- Server (local socket): no token 401, wrong token 401, a foreign `Origin` 403, GET 405, a good request 200 with JSON, a notification 202.
- Host (the real chrome and viewport, no GPU): `run_action` of Select All selects, Redo is refused with "Nothing to redo" (the start cube can already be undone), Make Group then Undo goes through the history, `get_model` counts faces.
- The token is 32 hex characters and changes on New Token; the settings read back.

## Out of scope (part 2)
Opening a replayed session in a tab; watching the model for changes (subscriptions); resources and prompts.

## Acceptance
- [ ] Claude Code connects with the copied command, reads the model and runs actions (owner, on Windows).
- [x] PLAN and NOTES are updated.
