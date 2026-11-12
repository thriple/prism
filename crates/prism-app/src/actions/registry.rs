//! The action registry: every tool, command and setting, with its menu path,
//! default key, "enabled when" rule, and whether Rust can run it yet.

use std::collections::HashMap;

use super::table::{self, ACTIONS, BEHAVIOURS, DOCK, MENUBAR};
use super::table_types::Item;

/// What happens when an action runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// Not built yet: shown greyed out, cannot run.
    NotBuilt,
    /// Picks a tool. The tools themselves come in later chunks.
    Tool,
    /// Sends an event that later chunks will handle (New, Select All…).
    Event,
    /// Done in this app now (opens a window, toggles a panel).
    Command,
}

/// When an action can run, beyond being built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Always,
    CanUndo,
    CanRedo,
    HasSelection,
    /// Groups are selected (Explode).
    HasGroups,
    /// A recent-file slot: enabled while that many files are remembered.
    Recent(usize),
    /// A label only ("No recent files").
    Never,
}

/// Whether a menu item shows a check mark, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    None,
    /// Checked while this tool is the active one.
    ActiveTool,
    /// Checked while its setting is on.
    Toggle,
}

/// Modes (PLAN 1.11) are not built yet; every action applies in all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    All,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::All => "All modes",
        }
    }
}

/// One registered action.
#[derive(Clone, Debug)]
pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    /// Menus leading to it, top first ("Edit", "Hide & Lock").
    pub menu_path: Vec<&'static str>,
    /// Default key in the SketchUp preset, in the prototype's notation.
    pub default_key: &'static str,
    pub run: Run,
    pub when: When,
    pub check: Check,
    pub mode: Mode,
}

impl Action {
    /// Built means Rust does what the action promises. Picking a tool whose
    /// behaviour is not built yet, or sending an event nobody handles, is not.
    pub fn built(&self) -> bool {
        self.run != Run::NotBuilt
    }
}

/// What the "enabled when" rules look at.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    /// Groups are selected.
    pub has_groups: bool,
    /// How many recent files are remembered.
    pub recent: usize,
}

/// Actions Rust runs in this chunk (opens a window, toggles a panel).
const COMMANDS: [&str; 24] = [
    // MCP Server (brief 21).
    "help.mcp",
    // Updates (brief 20).
    "help.update",
    // Toolbars (brief 19).
    "view.toolbars",
    "view.edittoolbars",
    "view.resettoolbars",
    // View Settings: these open the View Settings window.
    "view.sunset",
    "view.settings",
    "view.linew",
    "view.outline",
    "view.edges",
    "view.faces",
    "view.styles",
    "win.health",
    "win.materials",
    "win.components",
    "win.outliner",
    "win.info",
    "win.tags",
    "help.features",
    "help.search",
    "help.keys",
    "win.hotkeys",
    "help.debug",
    "help.themedemo",
];
/// Prototype commands kept as events for later chunks.
const EVENTS: [&str; 8] = [
    "cmd:new",
    "cmd:undo",
    "cmd:redo",
    "cmd:delete",
    "cmd:selall",
    "cmd:desel",
    "cmd:extents",
    "cmd:shadow",
];
/// Settings that show a check mark while on.
const TOGGLES: [&str; 19] = [
    "view.hidden",
    "view.wireframe",
    "view.basecolour",
    "view.colour",
    "view.textures",
    "light.sun",
    "light.default",
    "view.shadow",
    "view.guides",
    "show.faces",
    "show.lines",
    "show.points",
    "view.axes",
    "view.ground",
    "view.grid",
    "view.xray",
    "help.debug",
    "help.themedemo",
    "help.record",
];

/// Decide how an imported action runs in Rust.
fn run_of(id: &str, proto_run: &str) -> Run {
    if COMMANDS.contains(&id) {
        Run::Command
    } else if id.starts_with("help.scene.")
        || id == "help.record"
        || id == "view.guides"
        // View Settings: the toggles are handled by the session.
        || prism_view::view_style::TOGGLES.contains(&id)
        || id.starts_with("file.")
        || id == "exp.3mf"
        || id == "exp.gltf"
        || id == "isect.model"
        || id == "isect.sel"
        // Groups & Components part 1: handled by the session.
        || id == "edit.group"
        || id == "edit.explode"
        // Part 2: components and mirroring.
        || id == "edit.comp"
        || id == "edit.unique"
        // Model Health.
        || id == "edit.revface"
        || id == "edit.orient"
        // Basic Materials 1c.
        || id == "edit.selmat"
        // Basic Materials part 3.
        || id == "tex.reset"
        // Freeform NURBS part 3: cage editing.
        || id == "cage.edit"
        || id == "cage.add"
        // Freeform NURBS part 4.
        || id == "surf.loft"
        || id == "surf.revolve"
        // Freeform NURBS part 5.
        || id == "surf.sweep"
        || id == "surf.patch"
        // Exact Curves 4c.
        || id == "edit.soften"
        // Part 3: hiding.
        || matches!(
            id,
            "edit.hide" | "edit.unhide" | "edit.unhideall" | "edit.unhidelast" | "edit.isolate"
        )
        || id.starts_with("flip.")
        || id == "win.nexttab"
        || id == "win.prevtab"
    {
        // The file actions (brief 08) are handled by the app.
        // Test scenes for the viewport (brief 04), handled by the app.
        Run::Event
    } else if prism_tools::session::TOOLS.contains(&id) {
        // Only tools the session really runs can be picked (DECISIONS #20):
        // the rest stay greyed until they are built, even partly.
        Run::Tool
    } else if EVENTS.contains(&proto_run) {
        Run::Event
    } else {
        // Includes Smart Guides (built with Inference & Drawing, NOTES.md) and
        // the prototype's own Feel Panel.
        Run::NotBuilt
    }
}

fn when_of(rule: &str) -> When {
    match rule {
        "undo" => When::CanUndo,
        "redo" => When::CanRedo,
        "sel" => When::HasSelection,
        _ => When::Always,
    }
}

/// The registry: every action, in the prototype's order.
pub struct Registry {
    actions: Vec<Action>,
    index: HashMap<&'static str, usize>,
}

impl Registry {
    pub fn new() -> Self {
        let mut paths: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
        fn walk(
            items: &'static [Item],
            path: &mut Vec<&'static str>,
            out: &mut HashMap<&'static str, Vec<&'static str>>,
        ) {
            for item in items {
                match item {
                    Item::Action(id) => {
                        out.entry(id).or_insert_with(|| path.clone());
                    }
                    Item::Sub(name, sub) => {
                        path.push(name);
                        walk(sub, path, out);
                        path.pop();
                    }
                    Item::Separator => {}
                }
            }
        }
        for (menu, items) in MENUBAR {
            walk(items, &mut vec![*menu], &mut paths);
        }

        let actions: Vec<Action> = ACTIONS
            .iter()
            .map(|r| {
                let run = run_of(r.id, r.proto_run);
                Action {
                    id: r.id,
                    name: r.name,
                    menu_path: paths.get(r.id).cloned().unwrap_or_default(),
                    default_key: r.key,
                    run,
                    when: match r.id.strip_prefix("file.recent") {
                        Some("") => When::Never,
                        Some(n) => n.parse().map_or(When::Always, When::Recent),
                        None if matches!(
                            r.id,
                            "isect.sel"
                                | "edit.group"
                                | "edit.comp"
                                | "edit.revface"
                                | "edit.soften"
                        ) =>
                        {
                            When::HasSelection
                        }
                        None if r.id == "edit.explode"
                            || r.id == "edit.unique"
                            || r.id == "edit.hide"
                            || r.id == "edit.isolate"
                            || r.id.starts_with("flip.") =>
                        {
                            When::HasGroups
                        }
                        None => when_of(r.when),
                    },
                    check: if run == Run::Tool {
                        Check::ActiveTool
                    } else if TOGGLES.contains(&r.id) {
                        Check::Toggle
                    } else {
                        Check::None
                    },
                    mode: Mode::All,
                }
            })
            .collect();
        let index = actions.iter().enumerate().map(|(i, a)| (a.id, i)).collect();
        Self { actions, index }
    }

    pub fn get(&self, id: &str) -> Option<&Action> {
        self.index.get(id).map(|&i| &self.actions[i])
    }

    pub fn all(&self) -> &[Action] {
        &self.actions
    }

    /// Whether the action can run now.
    pub fn enabled(&self, id: &str, ctx: &Context) -> bool {
        self.get(id).is_some_and(|a| {
            a.run != Run::NotBuilt
                && match a.when {
                    When::Always => true,
                    When::CanUndo => ctx.can_undo,
                    When::CanRedo => ctx.can_redo,
                    When::HasSelection => ctx.has_selection,
                    When::HasGroups => ctx.has_groups,
                    When::Recent(n) => n <= ctx.recent,
                    When::Never => false,
                }
        })
    }

    /// Why an action cannot run, for its tooltip.
    pub fn why_disabled(&self, id: &str, ctx: &Context) -> Option<&'static str> {
        let a = self.get(id)?;
        if a.run == Run::NotBuilt {
            return Some("Not built yet");
        }
        match a.when {
            When::CanUndo if !ctx.can_undo => Some("Nothing to undo"),
            When::CanRedo if !ctx.can_redo => Some("Nothing to redo"),
            When::HasSelection if !ctx.has_selection => Some("Select something first"),
            When::HasGroups if !ctx.has_groups => Some("Select a group first"),
            When::Recent(n) if n > ctx.recent => Some("No file here yet"),
            When::Never => Some("Files you open or save are listed here"),
            _ => None,
        }
    }

    /// The top-level menus.
    pub fn menubar(&self) -> &'static [(&'static str, &'static [Item])] {
        MENUBAR
    }

    /// The tool dock's groups.
    pub fn dock(&self) -> &'static [(&'static str, &'static [&'static str])] {
        DOCK
    }

    /// The icon drawing for an action, if it has one.
    pub fn icon(&self, id: &str) -> Option<&'static str> {
        table::ICONS
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, svg)| *svg)
    }

    /// All icons, for checking that each one draws.
    pub fn icons(&self) -> &'static [(&'static str, &'static str)] {
        table::ICONS
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// An owned menu entry, for menus built at run time (the context menu).
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Action(&'static str),
    Separator,
    Sub(&'static str, Vec<Entry>),
}

impl Entry {
    pub fn from_items(items: &'static [Item]) -> Vec<Entry> {
        items
            .iter()
            .map(|i| match i {
                Item::Action(id) => Entry::Action(id),
                Item::Separator => Entry::Separator,
                Item::Sub(name, sub) => Entry::Sub(name, Entry::from_items(sub)),
            })
            .collect()
    }
}

/// What is under the cursor or selected, for the right-click menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub faces: usize,
    pub edges: usize,
    pub points: usize,
    pub groups: usize,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.faces + self.edges + self.points + self.groups == 0
    }

    /// The menu's heading: "Empty space", "1 face", "2 faces, 3 edges".
    pub fn label(&self) -> String {
        let parts: Vec<String> = [
            (self.groups, "group"),
            (self.faces, "face"),
            (self.edges, "edge"),
            (self.points, "point"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| format!("{n} {w}{}", if n > 1 { "s" } else { "" }))
        .collect();
        if parts.is_empty() {
            "Empty space".to_owned()
        } else {
            parts.join(", ")
        }
    }
}

/// The right-click menu for a selection, following the prototype's `onContext`.
pub fn context_menu(sel: Selection) -> Vec<Entry> {
    use Entry::{Action as A, Separator as S, Sub};
    if sel.is_empty() {
        return vec![
            A("edit.paste"),
            A("edit.pasteip"),
            S,
            A("edit.selall"),
            A("edit.unhide"),
            S,
            A("edit.undo"),
            A("edit.redo"),
            S,
            A("cam.extents"),
            A("win.info"),
        ];
    }
    let mut items = vec![
        A("edit.cut"),
        A("edit.copy"),
        A("edit.delete"),
        S,
        A("edit.hide"),
        A("edit.isolate"),
        A("edit.lock"),
        S,
        A("edit.group"),
        A("edit.comp"),
        A("edit.explode"),
        A("edit.unique"),
        S,
        Sub("Flip Along", vec![A("flip.x"), A("flip.y"), A("flip.z")]),
        Sub(
            "Intersect Faces",
            vec![A("isect.model"), A("isect.sel"), A("isect.ctx")],
        ),
    ];
    if sel.faces > 0 {
        items.extend([
            A("edit.revface"),
            A("edit.orient"),
            Sub("Select", vec![A("edit.selmat")]),
            Sub("Texture", vec![A("tool.texture"), A("tex.reset")]),
        ]);
    }
    if sel.edges > 0 {
        items.extend([A("edit.soften"), A("edit.weld")]);
    }
    items.extend([
        Sub("Cage", vec![A("cage.edit"), A("cage.add")]),
        Sub("Convert", vec![A("conv.curve"), A("conv.patch")]),
        S,
        A("edit.center"),
        A("cam.zoomsel"),
        A("edit.desel"),
        A("win.info"),
    ]);
    items
}

/// One line in the Planned Features window.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub name: &'static str,
    pub built: bool,
}

/// The Planned Features list: actions grouped by top-level menu (each once),
/// then the behaviours. Built means built in this Rust app.
pub fn planned_features(reg: &Registry) -> Vec<(&'static str, Vec<Feature>)> {
    let mut seen = std::collections::HashSet::new();
    seen.insert("file.recent"); // a placeholder, not a feature
    let mut groups = Vec::new();
    for (menu, items) in MENUBAR {
        let mut out = Vec::new();
        collect(items, &mut seen, &mut out);
        let feats = out
            .into_iter()
            .filter_map(|id| reg.get(id))
            .map(|a| Feature {
                name: a.name,
                built: a.built(),
            })
            .collect();
        groups.push((*menu, feats));
    }
    let behaviours = BEHAVIOURS
        .iter()
        .map(|(name, _prototype_has_it)| Feature {
            name,
            built: BUILT_BEHAVIOURS.contains(name),
        })
        .collect();
    groups.push(("Behaviours and feel", behaviours));
    groups
}

/// Behaviours from the prototype's list that the Rust app has now.
const BUILT_BEHAVIOURS: [&str; 21] = [
    "Groups and components, open and exit",
    "Command search (Shift+S)",
    "Inference: endpoint, midpoint, on-edge, on-face, guides",
    "Hover-wake anchors and guide intersections",
    "Axis locks (arrows and Shift)",
    "Typed values in the measurement box",
    "Selectable points, edges and faces",
    "Snap target highlight (point and edge)",
    "No overlapping coplanar faces: overlaps are split, faces with holes",
    "Size-relative tolerance (mm to km)",
    "Esc cancels, then backs out",
    "Zoom to cursor, orbit around the point under the cursor",
    "Box select: window (left to right) and crossing (right to left), Shift toggles, Ctrl adds",
    "Keymap presets (SketchUp, Blender, Fusion)",
    "Push/Pull: stretch, extrude, Alt new loop",
    "Live ghost previews",
    "Tool memory: double-click or Enter repeats the last value",
    // Freeform NURBS parts 1 and 2: curves and surfaces are in the model.
    "Exact circles, arcs and NURBS",
    "Model Health: reversed faces, holes, interior faces, nesting counts",
    "Save, open and export",
    "Trackpad gestures",
];

fn collect(
    items: &'static [Item],
    seen: &mut std::collections::HashSet<&'static str>,
    out: &mut Vec<&'static str>,
) {
    for item in items {
        match item {
            Item::Action(id) => {
                if seen.insert(id) {
                    out.push(id);
                }
            }
            Item::Sub(_, sub) => collect(sub, seen, out),
            Item::Separator => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids_in(items: &[Entry], out: &mut Vec<&'static str>) {
        for e in items {
            match e {
                Entry::Action(id) => out.push(id),
                Entry::Sub(_, sub) => ids_in(sub, out),
                Entry::Separator => {}
            }
        }
    }

    #[test]
    fn every_action_has_a_menu_path() {
        let reg = Registry::new();
        let missing: Vec<_> = reg
            .all()
            .iter()
            .filter(|a| a.menu_path.is_empty())
            .map(|a| a.id)
            .collect();
        assert!(missing.is_empty(), "no menu path: {missing:?}");
    }

    /// View Settings part 1: the View menu's items work, except lighting
    /// and hidden geometry (part 2) and section display.
    /// Every behaviour ticked as built is one the Planned Features window
    /// lists (a misspelt name would stay unticked).
    #[test]
    fn every_built_behaviour_is_listed() {
        for b in BUILT_BEHAVIOURS {
            assert!(BEHAVIOURS.iter().any(|(n, _)| *n == b), "{b} is not listed");
        }
    }

    #[test]
    fn the_view_menu_is_built() {
        let reg = Registry::new();
        for id in prism_view::view_style::TOGGLES.into_iter().chain([
            "view.settings",
            "view.linew",
            "view.outline",
            "view.edges",
            "view.faces",
            "view.styles",
            "view.shadow",
            "view.sunset",
            "light.sun",
            "light.default",
            "view.hidden",
            "edit.unhide",
        ]) {
            assert!(reg.get(id).is_some_and(Action::built), "{id} is greyed");
        }
        // View Settings (with Lighting) is listed with the other windows.
        let (_, window) = reg
            .menubar()
            .iter()
            .find(|(n, _)| *n == "Window")
            .expect("a Window menu");
        let mut ids = Vec::new();
        ids_in(&Entry::from_items(window), &mut ids);
        assert!(ids.contains(&"view.settings"));
    }

    #[test]
    fn menus_dock_and_context_only_name_registered_actions() {
        let reg = Registry::new();
        let mut ids = Vec::new();
        for (_, items) in reg.menubar() {
            ids_in(&Entry::from_items(items), &mut ids);
        }
        for (_, group) in reg.dock() {
            ids.extend(group.iter());
        }
        for sel in [
            Selection::default(),
            Selection {
                faces: 1,
                edges: 2,
                points: 0,
                groups: 1,
            },
        ] {
            ids_in(&context_menu(sel), &mut ids);
        }
        for (id, _) in reg.icons() {
            ids.push(id);
        }
        let unknown: Vec<_> = ids.into_iter().filter(|id| reg.get(id).is_none()).collect();
        assert!(unknown.is_empty(), "not in the registry: {unknown:?}");
    }

    #[test]
    fn ids_are_unique_and_the_import_is_complete() {
        let reg = Registry::new();
        let mut ids: Vec<_> = reg.all().iter().map(|a| a.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate ids");
        assert!(n > 180, "only {n} actions");
        assert!(reg.get("help.smartdemo").is_none(), "prototype-only helper");
        for id in [
            "file.docsettings",
            "help.pen",
            "script.console",
            "view.smart",
        ] {
            assert!(reg.get(id).is_some(), "{id} missing");
        }
        assert!(reg.menubar().iter().any(|(m, _)| *m == "Scripting"));
    }

    /// Exact Curves 6e: the curve tools are active, with menu paths, on
    /// the dock, and with their hotkeys; none is greyed any more.
    #[test]
    fn the_curve_tools_are_active() {
        let reg = Registry::new();
        let ctx = Context::default();
        for id in [
            "tool.circle",
            "tool.polygon",
            "tool.arc",
            "arc.2",
            "arc.3",
            "arc.c",
            "arc.p",
            "tool.fillet",
            "tool.followme",
        ] {
            let a = reg.get(id).expect(id);
            assert!(a.built() && reg.enabled(id, &ctx), "{id} is greyed");
            assert_eq!(a.run, Run::Tool, "{id}");
            assert!(!a.menu_path.is_empty(), "{id} has no menu path");
        }
        let docked: Vec<&str> = reg
            .dock()
            .iter()
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        for id in ["tool.circle", "tool.followme", "tool.fillet"] {
            assert!(docked.contains(&id), "{id} is not on the dock");
        }
        assert_eq!(reg.get("tool.circle").map(|a| a.default_key), Some("C"));
        assert_eq!(reg.get("tool.arc").map(|a| a.default_key), Some("A"));
    }

    /// Basic Materials 1b: the Paint Bucket is built (B, on the dock).
    #[test]
    fn the_paint_bucket_is_active() {
        let reg = Registry::new();
        let ctx = Context::default();
        let a = reg.get("tool.paint").expect("tool.paint");
        assert!(a.built() && reg.enabled("tool.paint", &ctx));
        assert_eq!(a.run, Run::Tool);
        assert_eq!(a.default_key, "B");
        assert!(!a.menu_path.is_empty());
        assert!(
            reg.dock()
                .iter()
                .any(|(_, ids)| ids.contains(&"tool.paint"))
        );
    }

    #[test]
    fn greyed_actions_cannot_run_and_say_why() {
        let reg = Registry::new();
        let ctx = Context::default();
        assert!(!reg.enabled("solid.union", &ctx));
        assert_eq!(reg.why_disabled("solid.union", &ctx), Some("Not built yet"));
        assert!(
            !reg.enabled("view.smart", &ctx),
            "Smart Guides wait for Inference & Drawing"
        );
        assert!(!reg.enabled("edit.undo", &ctx));
        assert_eq!(reg.why_disabled("edit.undo", &ctx), Some("Nothing to undo"));
        let ctx = Context {
            can_undo: true,
            ..ctx
        };
        assert!(reg.enabled("edit.undo", &ctx));
        assert!(reg.enabled("tool.line", &ctx));
        assert!(reg.enabled("help.features", &ctx));
        assert!(!reg.enabled("no.such.action", &ctx));
    }

    #[test]
    fn context_menu_follows_the_selection() {
        let ids = |s| {
            let mut v = Vec::new();
            ids_in(&context_menu(s), &mut v);
            v
        };
        let empty = ids(Selection::default());
        assert!(empty.contains(&"edit.paste") && !empty.contains(&"edit.delete"));
        let face = ids(Selection {
            faces: 1,
            ..Default::default()
        });
        assert!(face.contains(&"edit.revface") && !face.contains(&"edit.weld"));
        let edge = ids(Selection {
            edges: 2,
            ..Default::default()
        });
        assert!(edge.contains(&"edit.weld") && !edge.contains(&"edit.revface"));
        assert_eq!(
            Selection {
                faces: 1,
                edges: 3,
                points: 0,
                groups: 0,
            }
            .label(),
            "1 face, 3 edges"
        );
        assert_eq!(Selection::default().label(), "Empty space");
    }

    #[test]
    fn planned_features_cover_every_menu_action_once() {
        let reg = Registry::new();
        let groups = planned_features(&reg);
        let total: usize = groups.iter().map(|(_, f)| f.len()).sum();
        let built: usize = groups
            .iter()
            .flat_map(|(_, f)| f)
            .filter(|f| f.built)
            .count();
        let menu_actions = reg.all().iter().filter(|a| a.id != "file.recent").count();
        assert_eq!(total, menu_actions + BEHAVIOURS.len());
        assert!(built >= COMMANDS.len());
        assert!(built < total);
    }
}
