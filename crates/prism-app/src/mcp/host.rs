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
            "paint" => self.paint(args)?,
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
            "materials": s.material_rows().into_iter().map(|r| json!({
                "id": r.id.0,
                "name": r.name,
                "colour": format!("#{:02x}{:02x}{:02x}", r.srgba[0], r.srgba[1], r.srgba[2]),
                "opacity": f32::from(r.srgba[3]) / 255.0,
                "roughness": r.roughness,
                "metalness": r.metalness,
                "used": r.used,
                // A textured material's colour above is its image's average.
                "texture": r.texture.map(|t| json!({
                    "pixels": t.pixels,
                    "width_m": t.size[0],
                    "height_m": t.size[1],
                })),
            })).collect::<Vec<_>>(),
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
                material,
                ..
            } => json!({
                "kind": kind,
                "name": name,
                "tag": tag.map(|t| t.0),
                "hidden": hidden,
                "material": material,
            }),
            EntityInfo::Geometry {
                groups,
                faces,
                edges,
                points,
                area,
                materials,
            } => json!({
                "groups": groups,
                "faces": faces,
                "edges": edges,
                "points": points,
                "face_area": area,
                "materials": materials.map(|(f, b)| json!({"front": f, "back": b})),
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
                    // Material ids (get_model lists them); null is Default.
                    "front_material": f.paint.front.map(|m| m.0),
                    "back_material": f.paint.back.map(|m| m.0),
                });
                match f.surface.clone() {
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
                    // Exact Curves 5b, as placed in the world.
                    other => match other.transformed(&t).unwrap_or(other) {
                        prism_geom::Surface::Cone(c) => {
                            j["cone"] = json!({
                                "apex": point(c.apex),
                                "axis": point(c.axis),
                                "half_angle_degrees": round(c.half_angle.to_degrees()),
                                "outward": c.outward,
                            });
                        }
                        prism_geom::Surface::Sphere(x) => {
                            j["sphere"] = json!({
                                "centre": point(x.centre),
                                "axis": point(x.axis),
                                "radius": round(x.radius),
                                "outward": x.outward,
                            });
                        }
                        prism_geom::Surface::Torus(x) => {
                            j["torus"] = json!({
                                "centre": point(x.centre),
                                "axis": point(x.axis),
                                "major_radius": round(x.major),
                                "minor_radius": round(x.minor),
                                "outward": x.outward,
                            });
                        }
                        prism_geom::Surface::Nurbs(x) => {
                            let s = &x.surface;
                            j["nurbs"] = json!({
                                "degree": s.degree(),
                                "control_points": s.count(),
                                "closed": s.periodic(),
                                "u": [x.u.0, x.u.1],
                                "v": [x.v.0, x.v.1],
                                "faces_along_its_normal": !x.flipped,
                                "net": s.points().iter().map(|p| point(*p)).collect::<Vec<_>>(),
                            });
                        }
                        _ => {}
                    },
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
                let c = e.curve.transformed(&t).unwrap_or(e.curve.clone());
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
                } = &c
                {
                    j["arc"] = json!({
                        "centre": point(*centre),
                        "axis": [round(axis.x), round(axis.y), round(axis.z)],
                        "radius": round(*radius),
                        "runs": "counter-clockwise about the axis from a to b",
                    });
                }
                if let prism_geom::Curve::Spline(s) = &c {
                    j["spline"] = json!({
                        "degree": s.curve.degree(),
                        "closed": s.curve.is_closed(),
                        "control_points": s.curve.cage().iter().map(|p| point(*p)).collect::<Vec<_>>(),
                        "knots": s.curve.knots(),
                        "weights": s.curve.weights(),
                        "from": s.t0,
                        "to": s.t1,
                    });
                }
                if let Some(cid) = e.curve_id {
                    j["curve"] = json!(cid.0);
                }
                // Exact Curves 4c: soft edges are not drawn; shading runs
                // across smooth ones.
                if e.soft {
                    j["soft"] = json!(true);
                }
                if e.smooth {
                    j["smooth"] = json!(true);
                }
                // Exact Curves 6a: a fine segment standing in for a curve
                // that is not a line or arc (DECISIONS #32).
                if e.approx {
                    j["approx"] = json!(true);
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

    /// MCP `paint`: faces by id or the selection, with a material.
    fn paint(&mut self, args: &Value) -> Result<Value, String> {
        use prism_doc::FaceSide;
        use prism_tools::session::PaintWith;
        let s = &mut self.viewport.session;
        let sides: Vec<FaceSide> = match args.get("side").and_then(Value::as_str).unwrap_or("front")
        {
            "front" => vec![FaceSide::Front],
            "back" => vec![FaceSide::Back],
            "both" => vec![FaceSide::Front, FaceSide::Back],
            other => return Err(format!("side is front, back or both, not {other}.")),
        };
        let mut faces: Vec<prism_topo::FaceId> = args
            .get("faces")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_u64)
                    .map(|n| prism_topo::FaceId(n as u32))
                    .collect()
            })
            .unwrap_or_default();
        let mut groups = std::collections::BTreeSet::new();
        if args.get("selection").and_then(Value::as_bool) == Some(true) {
            faces.extend(s.selection().faces.iter().copied());
            groups = s.selected_groups().clone();
        }
        let with = if args.get("default").and_then(Value::as_bool) == Some(true) {
            PaintWith::Default
        } else if let Some(hex) = args.get("colour").and_then(Value::as_str) {
            let h = hex.trim_start_matches('#');
            let byte = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
            let (Some(r), Some(g), Some(b), 6) = (byte(0), byte(2), byte(4), h.len()) else {
                return Err(format!("colour is #rrggbb, not {hex}."));
            };
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(hex)
                .to_owned();
            PaintWith::New(prism_doc::Material::colour(&name, [r, g, b]))
        } else {
            let m = args.get("material");
            let id = match m {
                Some(Value::Number(n)) => n.as_u64().map(|n| prism_topo::MaterialId(n as u32)),
                Some(Value::String(name)) => s.material_named(name),
                _ => {
                    return Err(
                        "Say what to paint with: material (id or name), colour, or default.".into(),
                    );
                }
            };
            PaintWith::Existing(id.ok_or("No material with that name (get_model lists them).")?)
        };
        let set = faces
            .iter()
            .flat_map(|f| sides.iter().map(move |sd| (*f, *sd)))
            .collect();
        let m = s.paint_with(set, groups, with)?;
        Ok(json!({
            "painted_with": m.map(|m| m.0),
            "material": s.material_name(m),
            "model": self.model()["materials"],
        }))
    }

    fn use_tool(&mut self, args: &Value) -> Result<Value, String> {
        if let Some(tool) = args.get("tool").and_then(Value::as_str) {
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
                    // Keys held for this click: "ctrl", "shift", "alt".
                    let held: Vec<&str> = step
                        .get("keys")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    let has = |k: &str| held.contains(&k);
                    self.viewport.drive(Input::Press {
                        button: Button::Left,
                        x: s.x,
                        y: s.y,
                        shift: has("shift"),
                        ctrl: has("ctrl"),
                        alt: has("alt"),
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
    fn use_tool_draws_circles_and_polygons() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        // Active since Exact Curves 6e.
        assert!(h.chrome.enabled("tool.circle"));
        assert!(h.chrome.enabled("tool.polygon"));
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
            assert!(h.chrome.enabled(id), "{id} active since Exact Curves 6e");
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

    /// Freeform NURBS part 1: the Curve tool by clicks, and the curve in
    /// `get_geometry`.
    #[test]
    fn use_tool_draws_a_nurbs_curve() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        assert!(h.chrome.enabled("tool.curve"));
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.curve", "steps": [{"click": [0, 0, 0]}, {"click": [1, 2, 0]}, {"click": [3, 2, 0]}, {"click": [4, 0, 0]}, {"key": "Enter"}]}),
        ));
        let g = value(h.call("get_geometry", &json!({})));
        let edges = g["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1, "{g}");
        let s = &edges[0]["spline"];
        assert_eq!(s["degree"], 3, "{g}");
        assert_eq!(s["closed"], false);
        assert_eq!(s["control_points"].as_array().unwrap().len(), 4);
        assert_eq!(s["control_points"][1], json!([1.0, 2.0, 0.0]));
    }

    /// Exact Curves 5b: cones, spheres and tori are listed with their
    /// parameters.
    #[test]
    fn approximate_curves_show_in_geometry_and_health() {
        let mut m = prism_topo::Model::new();
        prism_ops::add_circle(&mut m, prism_math::DVec3::ZERO, prism_math::DVec3::Z, 1.0);
        let all = m.vertices().map(|(id, _)| id).collect();
        prism_ops::transform::scale(
            &mut m,
            &all,
            prism_math::DVec3::ZERO,
            prism_math::DVec3::new(2.0, 1.0, 1.0),
        );
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::with_model(m));
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let g = value(h.call("get_geometry", &json!({})));
        let edges = g["edges"].as_array().unwrap();
        assert!(
            !edges.is_empty() && edges.iter().all(|e| e["approx"] == true),
            "{g}"
        );
        let r = value(h.call("get_health", &json!({})));
        let row = r
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["check"] == "approx")
            .cloned()
            .expect("the row");
        assert_eq!(row["count"], 1, "one curve: {row}");
    }

    #[test]
    fn get_geometry_lists_cones_spheres_and_tori() {
        let mut m = prism_tools::scenes::sphere(prism_math::DVec3::new(0.0, 0.0, 5.0), 2.0);
        m.append(&prism_tools::scenes::cone(1.0, 2.0));
        m.append(&prism_tools::scenes::torus(3.0, 0.5));
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::with_model(m));
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let g = value(h.call("get_geometry", &json!({})));
        let of = |kind: &str| -> Vec<Value> {
            g["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|f| f.get(kind).cloned())
                .collect()
        };
        let (spheres, cones, tori) = (of("sphere"), of("cone"), of("torus"));
        assert_eq!((spheres.len(), cones.len(), tori.len()), (2, 2, 4), "{g}");
        assert_eq!(spheres[0]["radius"], 2.0);
        assert_eq!(spheres[0]["centre"], json!([0.0, 0.0, 5.0]));
        assert_eq!(cones[0]["apex"], json!([0.0, 0.0, 2.0]));
        assert_eq!(cones[0]["axis"], json!([0.0, 0.0, -1.0]));
        assert_eq!(tori[0]["major_radius"], 3.0);
        assert_eq!(tori[0]["minor_radius"], 0.5);
        assert_eq!(tori[0]["outward"], true);
    }

    #[test]
    fn use_tool_pulls_a_disc_into_a_cylinder() {
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
            &json!({"tool": "tool.circle", "steps": [{"click": [0, 0, 0]}, {"click": [0.5, 0, 0]}]}),
        ));
        assert_eq!(r["model"]["faces"], 1, "{r}");
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [0.1, 0.1, 0]}, {"move": [0.1, 0.1, 0.3]}, {"type": "1.5"}]}),
        ));
        assert_eq!(r["model"]["faces"], 4, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let sides: Vec<&Value> = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["cylinder"].is_object())
            .collect();
        assert_eq!(sides.len(), 2, "{g}");
        for f in sides {
            assert_eq!(f["cylinder"]["radius"], 0.5);
            assert_eq!(f["cylinder"]["axis"], json!([0.0, 0.0, 1.0]));
            assert_eq!(f["cylinder"]["outward"], true);
        }
        // Its two seams are soft and smooth (Exact Curves 4c).
        let soft = |g: &Value| -> usize {
            g["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["soft"] == true && e["smooth"] == true)
                .count()
        };
        assert_eq!(soft(&g), 2, "{g}");
        // Edit > Soften/Smooth reaches run_action: a top edge picked and
        // softened.
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.select", "steps": [{"click": [0.5, 0, 1.5]}]}),
        ));
        let r = value(h.call("run_action", &json!({"id": "edit.soften"})));
        let g = value(h.call("get_geometry", &json!({})));
        assert_eq!(soft(&g), 4, "the top circle's two halves too: {r}");
        // Push/Pull on the side gives a new radius (Exact Curves 4d).
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [0.3, -0.4, 0.75]}, {"move": [0.45, -0.6, 0.75]}, {"type": "0.25"}]}),
        ));
        assert_eq!(r["model"]["faces"], 4, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let radii: Vec<&Value> = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["cylinder"].get("radius"))
            .collect();
        assert_eq!(radii, [&json!(0.75), &json!(0.75)], "{g}");
    }

    #[test]
    fn intersect_faces_cuts_a_cylinder() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.circle", "steps": [{"click": [0, 0, 0]}, {"click": [0.5, 0, 0]}]}),
        ));
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [0.1, 0.1, 0]}, {"move": [0.1, 0.1, 0.3]}, {"type": "1.5"}]}),
        ));
        // An upright square through the cylinder, at y = 0.2: up, across
        // and down by typed lengths, then back to the start.
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.line", "steps": [
                {"click": [-1, 0.2, 0]}, {"move": [-1, 0.2, 1]}, {"type": "2"},
                {"move": [0, 0.2, 2]}, {"type": "2"},
                {"move": [1, 0.2, 1]}, {"type": "2"},
                {"click": [-1, 0.2, 0]}
            ]}),
        ));
        let cylinders = |h: &mut AppHost| -> usize {
            let g = value(h.call("get_geometry", &json!({})));
            g["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["cylinder"].is_object())
                .count()
        };
        assert_eq!(cylinders(&mut h), 2);
        value(h.call("run_action", &json!({"id": "isect.model"})));
        // Two lines down the side cut the half they cross in three
        // (Exact Curves 6b).
        assert_eq!(cylinders(&mut h), 4);
        // Nothing crosses any more and nothing is off its curve (the
        // square sticking through the solid leaves open and shared edges).
        let health = value(h.call("get_health", &json!({})));
        for row in health.as_array().unwrap() {
            if ["crossing", "arcs", "approx", "degenerate"]
                .contains(&row["check"].as_str().unwrap())
            {
                assert_eq!(row["count"], 0, "{health}");
            }
        }
    }

    #[test]
    fn a_line_drawn_down_a_cylinder_splits_it() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.circle", "steps": [{"click": [0, 0, 0]}, {"click": [0.5, 0, 0]}]}),
        ));
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [0.1, 0.1, 0]}, {"move": [0.1, 0.1, 0.3]}, {"type": "1.5"}]}),
        ));
        // From the bottom circle to the top one, straight up the side
        // (Exact Curves 6b-2: it sticks to the cylinder and splits it).
        let (x, y) = (0.5 * 0.5f64, -0.5 * 0.75f64.sqrt());
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.line", "steps": [
                {"click": [x, y, 0]}, {"move": [x, y, 0.7]}, {"type": "1.5"}
            ]}),
        ));
        let g = value(h.call("get_geometry", &json!({})));
        let sides = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["cylinder"].is_object())
            .count();
        assert_eq!(sides, 3, "{r}");
    }

    #[test]
    fn use_tool_fillets_a_box_edge() {
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::new());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.rect", "steps": [{"click": [0, 0, 0]}, {"click": [2, 2, 0]}]}),
        ));
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.push", "steps": [{"click": [1, 1, 0]}, {"move": [1, 1, 0.5]}, {"type": "2"}]}),
        ));
        assert_eq!(r["model"]["faces"], 6, "{r}");
        // Fillet (brief 26), active since Exact Curves 6e.
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.fillet", "steps": [{"click": [2, 0, 1]}, {"type": "0.25"}]}),
        ));
        assert_eq!(r["model"]["faces"], 7, "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let radii: Vec<&Value> = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["cylinder"].get("radius"))
            .collect();
        assert_eq!(radii, [&json!(0.25)], "{g}");
    }

    /// Follow Me (brief 26, step 5c): select a line, then click the profile.
    #[test]
    fn use_tool_follows_a_path_with_a_profile() {
        let mut m = prism_topo::Model::new();
        let c = [(-0.5, 1.5), (0.5, 1.5), (0.5, 2.5), (-0.5, 2.5)]
            .map(|(y, z)| prism_math::DVec3::new(0.0, y, z));
        for i in 0..4 {
            prism_ops::add_segment(&mut m, c[i], c[(i + 1) % 4]);
        }
        prism_ops::add_segment(
            &mut m,
            prism_math::DVec3::ZERO,
            prism_math::DVec3::new(3.0, 0.0, 0.0),
        );
        let (mut chrome, mut viewport, mut files) = parts(prism_doc::Document::with_model(m));
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        value(h.call("run_action", &json!({"id": "cam.extents"})));
        value(h.call(
            "use_tool",
            &json!({"tool": "tool.select", "steps": [{"click": [1.5, 0, 0]}]}),
        ));
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.followme", "steps": [{"click": [0, 0, 2]}]}),
        ));
        assert_eq!(r["model"]["faces"], 6, "a box along the line: {r}");
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
                alt: false,
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
                alt: false,
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

    /// MCP lists a textured material's image size and real size (Basic
    /// Materials 2c); its colour is the image's average.
    #[test]
    fn get_model_shows_textures() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/acceptance/files/textured.prism"
        );
        let c = prism_file::read(&std::fs::read(path).expect("golden")).expect("opens");
        let (mut chrome, mut viewport, mut files) =
            parts(prism_doc::Document::with_assembly(c.assembly));
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let m = value(h.call("get_model", &json!({})));
        let t = &m["materials"][0]["texture"];
        assert_eq!(t["width_m"], 0.4, "{m}");
        assert_eq!(t["height_m"], 0.2);
        assert!(t["pixels"][0].as_u64().is_some_and(|w| w > 0));
    }

    /// A PNG dropped on the window becomes a new textured material, picked
    /// for the Paint Bucket with the Materials window open (Basic Materials
    /// 2c); a file that is not really an image says so.
    #[test]
    fn a_dropped_image_becomes_a_textured_material() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/acceptance/files/textured.prism"
        );
        let c = prism_file::read(&std::fs::read(path).expect("golden")).expect("opens");
        let png = c
            .assembly
            .images()
            .values()
            .next()
            .expect("an image")
            .clone();
        let (mut chrome, mut viewport, mut files) = parts(prism_tools::scenes::cube());
        let o = |name: &str, bytes: Vec<u8>| crate::doc_files::Opened {
            name: name.into(),
            path: None,
            bytes,
        };
        files.dropped(
            o("bricks.png", png.bytes.to_vec()),
            &mut viewport.session,
            &mut chrome,
        );
        let rows = viewport.session.material_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "bricks");
        assert!(rows[0].texture.is_some());
        assert_eq!(chrome.active_tool, prism_tools::session::PAINT);
        assert!(chrome.panels.materials, "the Materials window opens");
        files.dropped(
            o("fake.jpg", b"no".to_vec()),
            &mut viewport.session,
            &mut chrome,
        );
        assert_eq!(viewport.session.material_rows().len(), 1, "nothing added");
        assert!(
            chrome
                .note
                .as_ref()
                .is_some_and(|(t, _)| t.starts_with("fake.jpg: "))
        );
    }

    /// MCP `paint` (Basic Materials 1c): a new colour on faces by id, one
    /// undo step; an existing material by name on the selection; the
    /// Paint Bucket through use_tool, with keys held.
    #[test]
    fn paint_colours_faces_and_the_bucket_takes_keys() {
        let (mut chrome, mut viewport, mut files) = parts(prism_tools::scenes::cube());
        let (mut tabs, mut tool) = (Vec::new(), "tool.select");
        let mut h = AppHost {
            chrome: &mut chrome,
            viewport: &mut viewport,
            files: &mut files,
            tab_actions: &mut tabs,
            last_tool: &mut tool,
        };
        let g = value(h.call("get_geometry", &json!({})));
        let faces = g["faces"].as_array().unwrap().clone();
        let top = faces
            .iter()
            .find(|f| f["normal"] == json!([0.0, 0.0, 1.0]))
            .expect("top")["id"]
            .clone();
        let r = value(h.call(
            "paint",
            &json!({"faces": [top], "colour": "#cc3322", "name": "Brick"}),
        ));
        assert_eq!(r["material"], "Brick", "{r}");
        let id = r["painted_with"].clone();
        let g = value(h.call("get_geometry", &json!({})));
        let painted: Vec<&Value> = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["front_material"] == id)
            .collect();
        assert_eq!(painted.len(), 1);
        let m = value(h.call("get_model", &json!({})));
        assert_eq!(m["materials"][0]["name"], "Brick");
        assert_eq!(m["materials"][0]["colour"], "#cc3322");
        assert_eq!(m["materials"][0]["used"], true);
        assert_eq!(m["materials"][0]["texture"], Value::Null);
        // One step: Undo takes the paint and the new material away.
        value(h.call("run_action", &json!({"id": "edit.undo"})));
        assert!(
            value(h.call("get_model", &json!({})))["materials"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        value(h.call("run_action", &json!({"id": "edit.redo"})));
        // By name, on the selection (every face), both sides.
        value(h.call("run_action", &json!({"id": "edit.selall"})));
        value(h.call(
            "paint",
            &json!({"selection": true, "material": "brick", "side": "both"}),
        ));
        let g = value(h.call("get_geometry", &json!({})));
        assert!(
            g["faces"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["front_material"] == id && f["back_material"] == id)
        );
        assert!(
            h.call("paint", &json!({"faces": [top], "material": "nothing"}))
                .is_err()
        );
        assert!(
            h.call("paint", &json!({"faces": [top], "colour": "red"}))
                .is_err()
        );
        // The bucket: Ctrl held fills every joined Default face.
        value(h.call(
            "paint",
            &json!({"selection": true, "default": true, "side": "both"}),
        ));
        value(h.call("run_action", &json!({"id": "edit.desel"})));
        value(h.call("run_action", &json!({"id": "cam.extents"})));
        let r = value(h.call(
            "use_tool",
            &json!({"tool": "tool.paint", "steps": [{"click": [0, 0, 2], "keys": ["ctrl"]}]}),
        ));
        assert!(r.get("clicks").is_some(), "{r}");
        let g = value(h.call("get_geometry", &json!({})));
        let filled = g["faces"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| !f["front_material"].is_null())
            .count();
        assert_eq!(filled, 6, "Ctrl filled every connected Default face");
    }
}
