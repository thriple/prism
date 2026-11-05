//! The shapes of the generated tables in `table.rs`.

/// One row of the prototype's action table, as imported.
#[derive(Clone, Copy, Debug)]
pub struct RawAction {
    pub id: &'static str,
    pub name: &'static str,
    /// Default key in the SketchUp preset, written as in the prototype ("Ctrl+Shift+S").
    pub key: &'static str,
    /// What the prototype ran ("tool:line", "cmd:undo"), or empty if it was not built there.
    pub proto_run: &'static str,
    /// The prototype's "enabled when" rule: "", "undo", "redo" or "sel".
    pub when: &'static str,
}

/// One entry in a menu.
#[derive(Clone, Copy, Debug)]
pub enum Item {
    Action(&'static str),
    Separator,
    Sub(&'static str, &'static [Item]),
}
