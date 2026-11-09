//! The tools, run on the main thread against the open document (brief
//! 21). Actions go through the registry exactly as a menu click does, so
//! every change is a command in the undo history.

use serde_json::{Value, json};

use prism_math::DVec3;
use prism_tools::{Button, Input, Key};

use super::protocol::{Host, Output};
use crate::chrome::{Chrome, Source};
use crate::files::Files;
use crate::viewport::Viewport;

pub struct AppHost<'a> {
    pub chrome: &'a mut Chrome,
    pub viewport: &'a mut Viewport,
    pub files: &'a mut Files,
    /// Next and previous tab, for the app to apply.
    pub tab_actions: &'a mut Vec<isize>,
    /// The tool the viewport has (kept in step when a tool is picked).
    pub last_tool: &'a mut &'static str,
}

/// Most faces and edges `get_geometry` lists unless asked.
const GEOMETRY_LIMIT: usize = 200;

/// A number for reading: six decimals is a micrometre.
fn round(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn point(p: DVec3) -> Value {
    json!([round(p.x), round(p.y), round(p.z)])
}

fn parse_point(v: &Value) -> Option<DVec3> {
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    Some(DVec3::new(a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?))
}

impl Host for AppHost<'_> {
    fn call(&mut self, name: &str, args: &Value) -> Result<Output, String> {
        self.chrome.rules = crate::app::rules(self.viewport, self.chrome.recent.len());
        let v = match name {
            "get_model" => self.model(),
            "get_selection" => self.selection(),
            "get_health" => self.health(),
            "list_actions" => self.actions(args.get("query").and_then(Value::as_str)),
            "get_geometry" => self.geometry(
                args.get("limit")
                    .and_then(Value::as_u64)
                    .map_or(GEOMETRY_LIMIT, |n| n as usize),
            ),
            "use_tool" => self.use_tool(args)?,
            "replay_session" => replay(args)?,
            "run_action" => self.run(args.get("id").and_then(Value::as_str))?,
            // The app takes it after drawing the next frame.
            "screenshot" => return Err("Screenshots are taken by the app's window.".into()),
            _ => return Err(format!("Unknown tool: {name}")),
        };
        Ok(Output::Text(
            serde_json::to_string_pretty(&v).unwrap_or_default(),
        ))
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
        let units = s.number.units;
        let bounds = s
            .scene()
            .bounds
            .map(|(lo, hi)| json!({"min": point(lo), "max": point(hi)}));
        json!({
            "document": document,
            "units": {
                "shown_in": units.length.name(),
                "decimals": units.decimals,
                "coordinates": "Points and lengths in the tools are metres; typed values are read in the shown unit unless they name one (400mm, 2.5m, 3').",
            },
            "bounds": bounds,
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

    /// A tool picked from the chrome reaches the viewport now (the app
    /// would do it on the next frame).
    fn follow_tool(&mut self) {
        if self.chrome.active_tool != *self.last_tool {
            self.viewport.tool_changed(self.chrome.active_tool);
            *self.last_tool = self.chrome.active_tool;
        }
    }

    fn geometry(&self, limit: usize) -> Value {
        let s = &self.viewport.session;
        let m = s.work_model();
        let t = s.open_transform();
        let faces: Vec<Value> = m
            .faces()
            .take(limit)
            .map(|(id, f)| {
                let outer: Vec<Value> = m
                    .points(&f.outer)
                    .into_iter()
                    .map(|p| point(t.transform_point3(p)))
                    .collect();
                let mut j = json!({
                    "id": id.0,
                    "area": round(m.face_area(id)),
                    "outer": outer,
                    "holes": f.holes.len(),
                });
                match f.surface {
                    prism_geom::Surface::Plane(pl) => {
                        j["normal"] = point(t.transform_vector3(pl.normal).normalize_or_zero());
                    }
                    // A curved face (Exact Curves part 4), as placed in the
                    // world.
                    prism_geom::Surface::Cylinder(c) => {
                        let c = c.transformed(&t).unwrap_or(c);
                        j["cylinder"] = json!({
                            "origin": point(c.origin),
                            "axis": point(c.axis),
                            "radius": round(c.radius),
                            "outward": c.outward,
                        });
                    }
                }
                j
            })
            .collect();
        let edges: Vec<Value> = m
            .edges()
            .take(limit)
            .map(|(id, e)| {
                let (a, b) = (
                    t.transform_point3(m.vertex(e.a)),
                    t.transform_point3(m.vertex(e.b)),
                );
                // An arc as placed in the world (an even group scale keeps
                // it an arc).
                let c = e.curve.transformed(&t).unwrap_or(e.curve);
                let mut j = json!({
                    "id": id.0,
                    "a": point(a),
                    "b": point(b),
                    "length": round(c.length(a, b)),
                });
                if let prism_geom::Curve::Arc {
                    centre,
                    axis,
                    radius,
                } = c
                {
                    j["arc"] = json!({
                        "centre": point(centre),
                        "axis": [round(axis.x), round(axis.y), round(axis.z)],
                        "radius": round(radius),
                        "runs": "counter-clockwise about the axis from a to b",
                    });
                }
                if let Some(cid) = e.curve_id {
                    j["curve"] = json!(cid.0);
                }
                j
            })
            .collect();
        json!({
            "faces_total": m.face_count(),
            "edges_total": m.edge_count(),
            "faces": faces,
            "edges": edges,
            "note": "World coordinates in metres, for the open group's model (the whole model when no group is open). Areas are in square metres.",
        })
    }

    fn use_tool(&mut self, args: &Value) -> Result<Value, String> {
        if let Some(tool) = args.get("tool").and_then(Value::as_str) {
            // The Circle and Polygon tools stay greyed in the menus until
            // Exact Curves is done, but an assistant can use them (brief 23).
            if let Some(&greyed) = prism_tools::session::GREYED_TOOLS
                .iter()
                .find(|t| **t == tool)
            {
                self.chrome.active_tool = greyed;
                self.follow_tool();
                self.viewport.session.mark_all_dirty();
                return self
                    .use_tool(&json!({"steps": args.get("steps").cloned().unwrap_or_default()}));
            }
            let is_tool = self
                .chrome
                .reg
                .get(tool)
                .is_some_and(|a| a.run == crate::actions::registry::Run::Tool);
            if !is_tool {
                return Err(format!(
                    "{tool} is not a tool. list_actions shows them (tool.…)."
                ));
            }
            self.run(Some(tool))?;
        }
        // Clicks are projected through the camera where it is going.
        self.viewport.session.settle_camera();
        let steps = args
            .get("steps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut clicks = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let at = |key: &str| step.get(key).map(parse_point);
            if let Some(p) = at("click").or_else(|| at("move")) {
                let p =
                    p.ok_or_else(|| format!("Step {}: a point is [x, y, z] in metres.", i + 1))?;
                let view = self.viewport.session.view();
                let s = view
                    .to_screen(p)
                    .map(|(s, _)| s)
                    .filter(|s| s.x >= 0.0 && s.y >= 0.0 && s.x <= view.w && s.y <= view.h)
                    .ok_or_else(|| {
                        format!(
                            "Step {}: the point {p} is outside the view, so nothing more was done. Change the view first (run_action cam.extents, or a standard view such as cam.v.top).",
                            i + 1
                        )
                    })?;
                self.viewport.drive(Input::Move { x: s.x, y: s.y });
                if step.get("click").is_some() {
                    let landed = self.viewport.session.inference().map(
                        |f| json!({"point": point(f.point), "snapped_to": format!("{:?}", f.kind)}),
                    );
                    self.viewport.drive(Input::Press {
                        button: Button::Left,
                        x: s.x,
                        y: s.y,
                        shift: false,
                        ctrl: false,
                    });
                    self.viewport.drive(Input::Release {
                        button: Button::Left,
                    });
                    clicks.push(json!({"asked": point(p), "landed": landed}));
                }
            } else if let Some(text) = step.get("type").and_then(Value::as_str) {
                self.viewport.drive(Input::Text {
                    text: text.to_owned(),
                });
                self.viewport.drive(Input::Key { key: Key::Enter });
            } else if let Some(k) = step.get("key").and_then(Value::as_str) {
                let key = match k {
                    "Enter" => Key::Enter,
                    "Escape" => Key::Escape,
                    "Backspace" => Key::Backspace,
                    _ => {
                        return Err(format!(
                            "Step {}: keys are Enter, Escape or Backspace.",
                            i + 1
                        ));
                    }
                };
                self.viewport.drive(Input::Key { key });
            } else if let Some(axis) = step.get("lock").and_then(Value::as_str) {
                let key = match axis {
                    "red" => Key::Right,
                    "green" => Key::Left,
                    "blue" => Key::Up,
                    _ => return Err(format!("Step {}: lock red, green or blue.", i + 1)),
                };
                self.viewport.drive(Input::Key { key });
            } else {
                return Err(format!(
                    "Step {}: use click, move, type, key or lock.",
                    i + 1
                ));
            }
        }
        self.viewport.session.mark_all_dirty();
        let s = &self.viewport.session;
        let (value, unit, _) = s.measurement();
        let m = s.work_model();
        Ok(json!({
            "tool": self.chrome.active_tool,
            "clicks": clicks,
            "hint": s.hint(),
            "measurement": format!("{value} {unit}").trim().to_owned(),
            "in_operation": s.in_operation(),
            "model": {"faces": m.face_count(), "edges": m.edge_count(), "points": m.vertex_count()},
            "selection": self.counts(),
            "can_undo": self.viewport.can_undo(),
        }))
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
        self.follow_tool();
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

/// Play a recording in a session of its own.
fn replay(args: &Value) -> Result<Value, String> {
    let text = match (
        args.get("json").and_then(Value::as_str),
        args.get("path").and_then(Value::as_str),
    ) {
        (Some(j), _) => j.to_owned(),
        (None, Some(p)) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::fs::read_to_string(p).map_err(|e| format!("Could not read {p} ({e})."))?
            }
            #[cfg(target_arch = "wasm32")]
            {
                return Err(format!("Files cannot be read here ({p})."));
            }
        }
        (None, None) => return Err("Give a recording's path or its json.".into()),
    };
    let r: prism_tools::Recording = serde_json::from_str(&text)
        .map_err(|e| format!("That is not a recorded session ({e})."))?;
    let s = prism_tools::Session::replay(&r);
    let m = s.model();
    Ok(json!({
        "events": r.events.len(),
        "matches_recording": if r.result.is_empty() { Value::Null } else { json!(r.ends_with(m)) },
        "end": {"faces": m.face_count(), "edges": m.edge_count(), "points": m.vertex_count(), "tool": s.tool()},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Registry;
    use crate::actions::keymap::{Keymap, Platform, Preset};

    fn value(out: Result<Output, String>) -> Value {
        match out.expect("a result") {
            Output::Text(t) => serde_json::from_str(&t).expect("json"),
            Output::Image { .. } => panic!("text expected"),
        }
    }

    /// The chrome and a viewport on `doc`, 1000 by 700 points.
    fn parts(doc: prism_doc::Document) -> (Chrome, Viewport, Files) {
        let reg = Registry::new();
        let chrome = Chrome::with_keymap(
            Platform::Windows,
            Keymap::from_preset(Preset::SketchUp, &reg),
        );
        let mut viewport = Viewport::with_document(doc);
        viewport.drive(Input::Resize {
            w: 1000.0,
            h: 700.0,
        });
        let files = Files::new(&viewport.session, 1);
        (chrome, viewport, files)
    }

    #[test]
    fn use_tool_draws_a_box_by_clicks_and_typing() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.rect", "steps": [{"click": [0, 0, 0]}, {"click": [2, 1, 0]}]}),
        ));
        assert_eq!(r["model"]["faces"], 1, "{r}");
        assert_eq!(r["clicks"][1]["landed"]["point"], json!([2.0, 1.0, 0.0]));
        // Push it up by a typed height.
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [1, 0.5, 0]}, {"move": [1, 0.5, 0.3]}, {"type": "1.5"}]}),
        ));
        assert_eq!(r["model"]["faces"], 6, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let top = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["normal"] == json!([0.0, 0.0, 1.0]))
            .expect("a top face");
        assert_eq!(top["area"], 2.0);
        assert!(top["outer"].as_array().unwrap().iter().all(|p| p[2] == 1.5));
        assert_eq!(*h.last_tool, "tool.push");
        // A point behind the camera stops the steps.
        let e = h
            .call("use_tool", &json!({"steps": [{"click": [0, -1000, 0]}]}))
            .unwrap_err();
        assert!(e.contains("outside the view"), "{e}");
        assert!(h.call("use_tool", &json!({"tool": "edit.selall"})).is_err());
        // Undo takes the push back.
        value(h.call("run_action", &json!({"id": "edit.undo"})));
        assert_eq!(value(h.call("get_model", &json!({})))["model"]["faces"], 1);
    }

    #[test]
    fn use_tool_draws_circles_and_polygons_that_stay_greyed_in_menus() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        assert!(
            !h.chrome.enabled("tool.circle"),
            "greyed until Exact Curves is done"
        );
        assert!(!h.chrome.enabled("tool.polygon"));
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.circle", "steps": [{"lock": "blue"}, {"click": [0, 0, 0]}, {"move": [0.5, 0, 0]}, {"type": "1"}]}),
        ));
        assert_eq!(r["model"]["faces"], 1, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let edges = g["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 2, "{g}");
        for e in edges {
            assert_eq!(e["arc"]["radius"], 1.0, "{e}");
            assert_eq!(e["arc"]["centre"], json!([0.0, 0.0, 0.0]));
            assert_eq!(e["curve"], edges[0]["curve"], "one curve");
        }
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.polygon", "steps": [{"type": "5s"}, {"lock": "blue"}, {"click": [4, 0, 0]}, {"click": [5, 0, 0]}]}),
        ));
        assert_eq!(r["model"]["faces"], 2, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let sides: Vec<&Value> = g["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e.get("arc").is_none())
            .collect();
        assert_eq!(sides.len(), 5);
        assert!(sides.iter().all(|e| e["curve"] == sides[0]["curve"]));
        assert!(!h.chrome.enabled("tool.circle"), "still greyed");
    }

    #[test]
    fn use_tool_draws_each_kind_of_arc() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        for id in ["arc.2", "arc.3", "arc.c", "arc.p", "tool.arc"] {
            assert!(
                !h.chrome.enabled(id),
                "{id} greyed until Exact Curves is done"
            );
        }
        let steps = [
            (
                "arc.2",
                json!([{"lock": "blue"}, {"click": [0, 0, 0]}, {"click": [1, 0, 0]}, {"move": [0.5, 0.2, 0]}, {"type": "0.5"}]),
            ),
            (
                "arc.3",
                json!([{"lock": "blue"}, {"click": [0, 1, 0]}, {"click": [0.5, 1.5, 0]}, {"click": [1, 1, 0]}]),
            ),
            (
                "arc.c",
                json!([{"lock": "blue"}, {"click": [-1, 0, 0]}, {"move": [-0.5, 0, 0]}, {"type": "0.5"}, {"move": [-1, 0.5, 0]}, {"type": "90"}]),
            ),
            (
                "arc.p",
                json!([{"lock": "blue"}, {"click": [-1, 1, 0]}, {"click": [-0.5, 1, 0]}, {"move": [-0.65, 1.35, 0]}, {"click": [-1, 1.5, 0]}]),
            ),
        ];
        for (tool, s) in steps {
            value(h.call("use_tool", &json!({"tool": tool, "steps": s})));
        }
        let g = value(h.call("get_geometry", &json!({})));
        let arcs: Vec<&Value> = g["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e.get("arc").is_some())
            .collect();
        let has = |c: [f64; 3]| {
            arcs.iter()
                .any(|e| e["arc"]["centre"] == json!(c) && e["arc"]["radius"] == json!(0.5))
        };
        assert_eq!(arcs.len(), 4, "{g}");
        assert!(has([0.5, 0.0, 0.0]), "the half circle: {g}");
        assert!(has([0.5, 1.0, 0.0]), "through three points: {g}");
        assert!(has([-1.0, 0.0, 0.0]), "by centre: {g}");
        assert!(has([-1.0, 1.0, 0.0]), "the pie: {g}");
        assert_eq!(g["faces_total"], 1, "the pie is a face");
    }

    #[test]
    fn geometry_lists_a_cylinder_face() {
        let mut doc = prism_doc::Document::new();
        doc.run(prism_doc::Command::Import {
            model: prism_tools::scenes::cylinder(1.5, 2.0),
        });
        let (mut chrome, mut viewport, mut files) = parts(doc);
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let g = value(h.call("get_geometry", &json!({})));
        let faces = g["faces"].as_array().unwrap();
        let sides: Vec<&Value> = faces.iter().filter(|f| f["cylinder"].is_object()).collect();
        assert_eq!(sides.len(), 2, "{g}");
        for f in sides {
            let c = &f["cylinder"];
            assert_eq!(c["radius"], 1.5);
            assert_eq!(c["axis"], json!([0.0, 0.0, 1.0]));
            assert_eq!(c["outward"], true);
            assert!(f["normal"].is_null(), "no single normal");
        }
        assert_eq!(faces.iter().filter(|f| f["normal"].is_array()).count(), 2);
    }

    #[test]
    fn geometry_lists_faces_up_to_a_limit() {
        let (mut chrome, mut viewport, mut files) = parts(prism_tools::scenes::cube());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let g = value(h.call("get_geometry", &json!({"limit": 2})));
        assert_eq!(g["faces_total"], 6);
        assert_eq!(g["faces"].as_array().unwrap().len(), 2);
        assert_eq!(g["edges"].as_array().unwrap().len(), 2);
        let all = value(h.call("get_geometry", &json!({})));
        for f in all["faces"].as_array().unwrap() {
            let n: f64 = f["normal"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_f64().unwrap().powi(2))
                .sum();
            assert!((n - 1.0).abs() < 1e-9, "unit normals");
        }
        assert_eq!(
            value(h.call("get_model", &json!({})))["units"]["shown_in"],
            "Metres"
        );
    }

    #[test]
    fn replay_says_whether_a_recording_matches() {
        let mut s = prism_tools::Session::new(prism_doc::Document::new());
        s.handle(&prism_tools::Event {
            t: 0.0,
            input: Input::Resize {
                w: 1000.0,
                h: 700.0,
            },
        });
        s.start_recording();
        for input in [
            Input::Tool {
                id: "tool.rect".into(),
            },
            Input::Move { x: 400.0, y: 400.0 },
            Input::Press {
                button: Button::Left,
                x: 400.0,
                y: 400.0,
                shift: false,
                ctrl: false,
            },
            Input::Release {
                button: Button::Left,
            },
            Input::Move { x: 600.0, y: 450.0 },
            Input::Press {
                button: Button::Left,
                x: 600.0,
                y: 450.0,
                shift: false,
                ctrl: false,
            },
            Input::Release {
                button: Button::Left,
            },
        ] {
            s.handle(&prism_tools::Event { t: 1.0, input });
        }
        let json = s.stop_recording().expect("recorded");
        let r = replay(&json!({"json": json})).expect("replays");
        assert_eq!(r["matches_recording"], true);
        assert_eq!(r["end"]["faces"], 1);
        let mut changed: Value = serde_json::from_str(&json).unwrap();
        changed["result"] = json!("{}");
        let r = replay(&json!({"json": changed.to_string()})).expect("replays");
        assert_eq!(r["matches_recording"], false);
        assert!(replay(&json!({"path": "/no/such/file.json"})).is_err());
        assert!(replay(&json!({})).is_err());
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
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
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
