# Prism

Prism is a 3D modelling program written in Rust. It aims for SketchUp's easy tool feel, with exact curved geometry underneath. A path-traced renderer and simulations come later.

It runs on Windows, macOS and in the browser.

## Download

The newest Windows and macOS builds are on the [latest release](https://github.com/thriple/prism/releases/tag/latest). Older builds stay on the Releases page as "Prism build N".

## Build it yourself

```bash
cargo run -p prism-app
```

For the web version, install [Trunk](https://trunkrs.dev) and run `trunk serve`, then open http://127.0.0.1:8080.

## Where to read more

- `PLAN.md`: the full plan and where the work stands
- `NOTES.md`: recent changes and loose ends
- `docs/`: decisions, briefs for each piece of work, and the UI style guide

## Try it in your browser

https://thriple.github.io/prism/
