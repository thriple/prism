# Chunk 10: View Settings (part 2: the sun, shadows and Show Hidden)

**Goal.** The model can be lit by a real sun, placed from a date, a time and a location, and faces cast shadows on each other and on the ground. Hidden things can be shown as ghosts to pick and unhide (PLAN 1.14, Lighting and Hidden geometry).

**Read first:** brief 17 and NOTES "View Settings (part 1)"; PLAN 1.14 (Lighting); `crates/prism-render/src/gpu.rs` and `viewport.wgsl`.

## Parts of this chunk
1. What is shown, lines and faces (built).
2. **The sun, shadows and Show Hidden** (this brief). View Settings is an M chunk, so two parts (PLAN 6.0a).

## Requirements
- **Lighting in the view style** (saved with the file, as part 1):
  - **Default lighting:** today's fixed light, unchanged.
  - **Sun:** a date (month and day), a time of day, a latitude and longitude, the time zone (hours from UTC), and the north angle (north is the green axis at 0°). The sun's direction comes from the standard NOAA solar position formulas, in `prism-math` (pure, f64).
  - **Light and dark:** how strong the lit side is, and how bright the shaded side stays.
  - **Shadows:** one switch (View > Lighting > Ground Shadow becomes "Shadows"). With the sun, faces cast shadows on faces and on the ground. With default lighting, only the ground shadow, as now.
  - **Night:** with the sun below the horizon, there is no direct light and no shadows, only the dark level.
- **Shadow map:** a depth pass of the faces from the sun, fitted to the model's bounds, kept relative to the eye like everything else (PLAN 4). Faces look it up with a comparison sampler and a few taps for a soft edge. Hidden objects cast nothing (hidden must be free). It must work on the web build (WebGL2 supports depth comparison).
- **The ground shadow** follows the sun's direction when the sun is on.
- **Show Hidden** (View > Hidden Geometry, a toggle in the view style): hidden groups, components and tags are drawn as faint dotted ghosts. Ghosts can be clicked and box-selected, and Edit > Unhide Selected shows them again. With the toggle off, hidden things cost nothing, as now.
- **Menus:** Lighting > Default Lighting and Sun (one or the other), Sun and Shadow Settings… (opens View Settings). The View Settings card gets a Lighting section.

## Out of scope
- Fog, ground opacity and "show below ground".
- Shadows from loose edges.
- The time-of-day slider in a toolbar.
- Soft shadows that change with the sun's size.

## Tests
- Sun position:
  - at the equator on the March equinox at solar noon, the sun is nearly overhead;
  - at 6:00 it rises in the east;
  - at the North Pole in December it is below the horizon;
  - a north angle of 90° turns the direction by 90°.
- Screenshots:
  - with the sun, a box on a plate darkens the plate where its shadow falls, and the same spot is lit with shadows off;
  - at night faces are at the dark level.
- The view style round-trips with its lighting; the Lighting menu items and Hidden Geometry are active.
- Session: with Show Hidden on, a hidden group can be clicked and unhidden; with it off, it cannot.

## Acceptance
- [ ] The sun lights the model from the date, time and place, and shadows fall on faces and the ground.
- [ ] Hidden things show as ghosts and can be unhidden.
- [ ] It runs on the web build.
- [ ] PLAN 6.0 and NOTES are updated.
