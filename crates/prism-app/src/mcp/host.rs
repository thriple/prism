//! The tools, run on the main thread against the open document (brief
//! 21). Actions go through the registry exactly as a menu click does, so
//! every change is a command in the undo history.

use serde_json::{Value, json};

use super::protocol::Host;
use crate::chrome::{Chrome, Source};
use crate::files::Files;
use crate::viewport::Viewport;

pub struct AppHost<'a> {
    pub chrome: &'a mut Chrome,
    pub viewport: &'a mut Viewport,
    pub files: &'a mut Files,
    /// Next and previous tab, for the app to apply.
    pub tab_actions: &'a mut Vec<isize>,
}

impl Host for AppHost<'_> {
    fn call(&mut self, name: &str, args: &Value) -> Result<String, String> {
        self.chrome.rules = crate::app::rules(self.viewport, self.chrome.recent.len());
        let v = match name {
            "get_model" => self.model(),
            "get_selection" => self.selection(),
            "get_health" => self.health(),
            "list_actions" => self.actions(args.get("query").and_then(Value::as_str)),
            "run_action" => self.run(args.get("id").and_then(Value::as_str))?,
            _ => return Err(format!("Unknown tool: {name}")),
        };
        Ok(serde_json::to_string_pretty(&v).unwrap_or_default())
    }
}

impl AppHost<'_> {
    fn counts(&self) -> Value {
        let c = self.viewport.counts();
        json!({"faces": c.faces, "edges": c.edges, "points": c.points, "groups": c.groups})
    }

    fn model(&self) -> Value {
        let s = &self.viewport.session;
        let m = s.work_model();
        let groups: Vec<Value> = s
            .outline()
            .into_iter()
            .map(|r| {
                json!({
                    "label": r.label,
                    "kind": r.kind,
                    "depth": r.depth,
                    "shown": r.shown,
                    "hidden": r.hidden,
                    "selected": r.selected,
                    "open": r.open,
                })
            })
            .collect();
        let document = self
            .chrome
            .docs
            .get(self.chrome.active_doc)
            .map(|(name, unsaved)| json!({"name": name, "unsaved": unsaved}));
        json!({
            "document": document,
            "tool": self.chrome.active_tool,
            "in_open_group": !s.open_path().is_empty(),
            "model": {
                "faces": m.face_count(),
                "edges": m.edge_count(),
                "points": m.vertex_count(),
                "note": "Counts are for the open group's model (the whole model when no group is open).",
            },
            "groups_and_components": groups,
            "selection": self.counts(),
            "view_style": s.view_style().name,
            "can_undo": self.viewport.can_undo(),
            "can_redo": self.viewport.can_redo(),
        })
    }

    fn selection(&self) -> Value {
        use prism_tools::session::EntityInfo;
        match self.viewport.session.entity_info() {
            EntityInfo::Instance {
                name,
                kind,
                tag,
                hidden,
                ..
            } => json!({
                "kind": kind,
                "name": name,
                "tag": tag.map(|t| t.0),
                "hidden": hidden,
            }),
            EntityInfo::Geometry {
                groups,
                faces,
                edges,
                points,
                area,
            } => json!({
                "groups": groups,
                "faces": faces,
                "edges": edges,
                "points": points,
                "face_area": area,
            }),
        }
    }

    fn health(&self) -> Value {
        let rows: Vec<Value> = self
            .viewport
            .session
            .health_rows()
            .into_iter()
            .map(|r| {
                json!({
                    "check": r.key,
                    "label": r.label,
                    "count": r.count,
                    "detail": r.detail,
                    "fix": r.fix,
                })
            })
            .collect();
        json!(rows)
    }

    fn actions(&self, query: Option<&str>) -> Value {
        let words: Vec<String> = query
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let rows: Vec<Value> = self
            .chrome
            .reg
            .all()
            .iter()
            .filter(|a| {
                let hay = format!("{} {} {}", a.id, a.name, a.menu_path.join(" ")).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .map(|a| {
                let mut row = json!({
                    "id": a.id,
                    "name": a.name,
                    "menu": a.menu_path.join(" > "),
                    "can_run": self.chrome.enabled(a.id),
                });
                let key = self.chrome.key_label(a.id);
                if !key.is_empty() {
                    row["key"] = json!(key);
                }
                if let Some(why) = self.chrome.reg.why_disabled(a.id, &self.chrome.rules) {
                    row["why_not"] = json!(why);
                }
                row
            })
            .collect();
        json!(rows)
    }

    fn run(&mut self, id: Option<&str>) -> Result<Value, String> {
        let id = id.ok_or("run_action needs an id (see list_actions).")?;
        let id = self
            .chrome
            .tool_id(id)
            .ok_or_else(|| format!("There is no action {id}. list_actions shows them."))?;
        if !self.chrome.enabled(id) {
            let name = self.chrome.reg.get(id).map_or(id, |a| a.name);
            let why = self
                .chrome
                .reg
                .why_disabled(id, &self.chrome.rules)
                .unwrap_or("it cannot run now");
            return Err(format!("{name} cannot run now: {why}."));
        }
        self.chrome.run(id, Source::Assistant);
        crate::app::dispatch(self.chrome, self.viewport, self.files, self.tab_actions);
        self.viewport.session.mark_all_dirty();
        self.chrome.rules = crate::app::rules(self.viewport, self.chrome.recent.len());
        Ok(json!({
            "ran": id,
            "status": self.chrome.status,
            "tool": self.chrome.active_tool,
            "selection": self.counts(),
            "can_undo": self.viewport.can_undo(),
            "can_redo": self.viewport.can_redo(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Registry;
    use crate::actions::keymap::{Keymap, Platform, Preset};

    fn value(text: Result<String, String>) -> Value {
        serde_json::from_str(&text.expect("a result")).expect("json")
    }

    #[test]
    fn tools_read_the_model_and_run_actions_with_undo() {
        let reg = Registry::new();
        let mut chrome = Chrome::with_keymap(
            Platform::Windows,
            Keymap::from_preset(Preset::SketchUp, &reg),
        );
        // The start scene: a cube.
        let mut viewport = Viewport::new();
        let mut files = Files::new(&viewport.session, 1);
        let mut tabs = Vec::new();
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
        };
        let none = json!({});
        let m = value(h.call("get_model", &none));
        assert_eq!(m["model"]["faces"], 6);
        assert_eq!(m["can_redo"], false);
        // Greyed actions are refused with the reason.
        let e = h
            .call("run_action", &json!({"id": "edit.redo"}))
            .unwrap_err();
        assert!(e.contains("Nothing to redo"), "{e}");
        assert!(h.call("run_action", &json!({"id": "edit.nope"})).is_err());
        assert!(h.call("run_action", &none).is_err());
        // Select All, as the menu does.
        let r = value(h.call("run_action", &json!({"id": "edit.selall"})));
        assert_eq!(r["selection"]["faces"], 6);
        assert_eq!(value(h.call("get_selection", &none))["faces"], 6);
        // Make Group, then undo it through the same history.
        value(h.call("run_action", &json!({"id": "edit.group"})));
        let m = value(h.call("get_model", &none));
        assert_eq!(m["model"]["faces"], 0, "the cube went into the group");
        assert_eq!(m["groups_and_components"].as_array().unwrap().len(), 1);
        assert_eq!(m["can_undo"], true);
        value(h.call("run_action", &json!({"id": "edit.undo"})));
        assert_eq!(value(h.call("get_model", &none))["model"]["faces"], 6);
        // The action list, filtered.
        let a = value(h.call("list_actions", &json!({"query": "select all"})));
        let row = a
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "edit.selall")
            .expect("listed");
        assert_eq!(row["can_run"], true);
        assert_eq!(row["key"], "Ctrl+A");
        // Model Health answers.
        assert!(value(h.call("get_health", &none)).as_array().is_some());
    }
}
