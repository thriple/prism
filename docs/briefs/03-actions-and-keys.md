# Chunk 3: Actions & Keys

**Goal.** One action registry, and everything generated from it: the menu bar, the tool dock, the context menu, command search, hotkeys, a hotkeys preferences window, and the Planned Features checklist.

**Read first:** PLAN.md sections 1.5, 1.6, 1.7, 1.12 and 3.1. The prototype shows the target look and the complete action list.

## Requirements
- **Registry.** Each action has an id, a name, a menu path, a default hotkey, a mode (all, or one), an "enabled when" rule, and an optional handler. An action with no handler is shown greyed out, never hidden. Start from the prototype's `ACT`, `MENUBAR`, `DOCK` and `IC` tables (see `docs/prototype/README.md`), which hold about 200 actions and their icons.
- **Menu bar, dock and context menu** are generated from the registry, in the Motion Kit's dark Soft glass style (`docs/UI-STYLE.md`), using egui. Follow the kit's `ContextMenu`, `CommandPalette`, `TooltipToolbar`, `Tooltip` and `Kbd` for the matching parts. The dock has groups and two columns. The context menu shows only what applies to the selection.
- **Command search** (Shift+S) finds any action by name.
- **Keymap engine.**
  - Bindings use physical key positions, so L, R and P stay put on other layouts, plus modifiers.
  - Ctrl on Windows is Cmd on macOS, shown correctly in menus.
  - Presets: SketchUp (the default), plus empty Blender and Fusion presets that can be filled in later.
  - Bindings are stored as JSON, and can be exported and imported.
- **Hotkeys window** (PLAN.md 1.12): a searchable table grouped like the menus, click-to-record, Esc to cancel, Backspace to clear, conflict detection with Replace, Keep both and Cancel, warnings for keys the browser takes in the web build, a modifiers tab, reset one or all.
- **Planned Features window:** generated from the registry, with built actions crossed out and a built over total count.
- **Esc rule** (PLAN.md 1.5): cancel an operation, else close a menu or window, else exit an opened group.
- **Dispatch only.** Actions raise events that later chunks handle. This chunk does not implement tools.

## Out of scope
Tool behaviour, the Mac system menu bar (later), mode switching, scenes, rendering of the 3D view.

## Acceptance
- [ ] A test fails if any registered action has no menu path.
- [ ] A test fails if the default SketchUp preset binds two actions to the same key in the same mode.
- [ ] Menus, dock, context menu and Planned Features all list the same actions as the registry, with no hand-written lists.
- [ ] Physical-key tests: the same binding fires on QWERTY and AZERTY layouts.
- [ ] Recording a conflicting key in the hotkeys window shows the other action and the three choices.
- [ ] Greyed actions cannot be run, and say why on hover ("Not in this prototype yet" for now).
- [ ] It matches `docs/UI-STYLE.md` and the kit's `soft.html` and `demo.html`, side by side, including spring motion and the reduce-motion setting.
