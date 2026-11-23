# MCP Server: remote access (owner request, 2026-11-23)

**Goal.** The MCP server (brief 21) was deliberately built local-only: it listens on `127.0.0.1` only, and refuses any request whose `Origin` header is not this machine's. The owner cannot use Claude Desktop (a security rule on their side) and needs an AI assistant running elsewhere to reach Prism, over a tunnel or a VPN they set up themselves. This is a considered reversal of brief 21's own scoping, not a bug fix, so it gets its own brief rather than quietly extending that one.

**Read first:** brief 21 (`docs/briefs/21-mcp-server.md`); `crates/prism-app/src/mcp/server.rs`, `mod.rs` and `crates/prism-app/src/chrome/mcp_window.rs`.

## Size
S.

## What changed
- **A second switch, "Allow connections from other devices" (`Settings::remote`, off by default)**, in the MCP Server window, below "Let AI assistants connect". Turning it on:
  - Listens on every interface (`0.0.0.0`) instead of `127.0.0.1` only.
  - Stops checking the `Origin` header at all. A legitimate remote caller's origin (a tunnel's own address, say) cannot be told from a stranger's by that header alone once the server is reachable from elsewhere on purpose, so the bearer token becomes the only guard, the same as a plain token-authenticated API.
  - Generates a fresh token at once, so a token that may already have been seen or copied while the server was local-only does not carry over into the wider exposure.
- **The window says so plainly** while remote is on: the "Listening on…" line changes to say it is reachable from other devices, with a second line in the warning colour ("Anyone who can reach this computer on that port and has the token can connect. Keep the token secret, and turn this off when you are done."), and the bottom disclaimer's wording changes to match.
- **What stays the same:** the token check itself (still `Authorization: Bearer <token>`, still 401 without it); the `claude mcp add` command shown (still `http://127.0.0.1:...`, for an assistant on this same machine); the protocol handling; the web build, where the server is still not available at all.

## What Prism does not do
Prism does not run or manage a tunnel itself. Getting a public, reachable address for the port it now listens on wider (a tunnel such as `cloudflared`, a VPN such as Tailscale, or a router's port forward) is the user's own choice and their own setup; Prism's only job here is to stop being artificially restricted to loopback once they have one. A dedicated "set up a tunnel from inside Prism" tool, if wanted later, is its own, separate piece of work (Shapes & Placement's Bend/Twist/Taper was already next in the plan; this was pulled in ahead of it at the owner's request).

## Tests
- `remote_connections_skip_the_origin_check_but_still_need_the_token` (prism-app, `mcp::server`): with `remote: true`, a request with a stranger's `Origin` succeeds; one with no token, or the wrong one, still 401s.
- `the_endpoint_checks_token_origin_and_method` (existing, unchanged behaviour): `remote: false` still refuses a stranger's `Origin`.
- `settings_read_back_and_default_off`: an old save with no `remote` row reads back as `remote: false` (local-only), so nobody's settings change exposure on an upgrade.

## Out of scope
A tunnel or VPN built into Prism; rate limiting or an audit log beyond the existing "last few calls"; token expiry; anything for the web build (still native only, same as brief 21).

## Acceptance
- [ ] The owner connects an assistant to Prism over a tunnel of their own choosing, reads the model and runs an action.
- [x] PLAN and NOTES are updated.
