//! GLB that keeps groups and components (brief 35, export for Beverly and
//! other game engines): named, nested nodes with their own axes, and one
//! shared mesh per component.

use prism_doc::{Change, Command, Document};
use prism_file::mesh::gltf::Scene;
use prism_file::mesh::{ExportOptions, Format, structured, to_painted_mesh};
use prism_math::{DMat4, DVec3};
use prism_ops::Selection;

fn all(m: &prism_topo::Model, from: &Selection) -> Selection {
    // What is new since `from`.
    Selection {
        vertices: m
            .vertices()
            .map(|(id, _)| id)
            .filter(|v| !from.vertices.contains(v))
            .collect(),
        edges: m
            .edges()
            .map(|(id, _)| id)
            .filter(|e| !from.edges.contains(e))
            .collect(),
        faces: m
            .faces()
            .map(|(id, _)| id)
            .filter(|f| !from.faces.contains(f))
            .collect(),
        guides: Default::default(),
    }
}

fn box_at(d: &mut Document, x: f64, y: f64, w: f64) -> Selection {
    let before = all(d.model(), &Selection::default());
    d.run(Command::Rect {
        x0: x,
        y0: y,
        x1: x + w,
        y1: y + w,
        z: 0.0,
    });
    let face = prism_ops::face_at(d.model(), DVec3::new(x + w / 2.0, y + w / 2.0, 0.0)).unwrap();
    let up = d.model().face(face).unwrap().plane().unwrap().normal.z > 0.0;
    d.run(Command::PushPull {
        face,
        distance: if up { w } else { -w },
        new_loop: false,
    });
    all(d.model(), &before)
}

/// A wall group holding two chunks both named "Chunk" (nested groups), a
/// component "Post" used three times (one mirrored), and a loose box.
fn document() -> Document {
    let mut d = Document::new();
    let chunk = d.assembly().next_instance();
    let sel = box_at(&mut d, 0.0, 0.0, 1.0);
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    let chunk2 = d.assembly().next_instance();
    let sel = box_at(&mut d, 1.0, 0.0, 1.0);
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    for c in [chunk, chunk2] {
        d.run(Command::NameInstance {
            instance: c,
            name: "Chunk".into(),
        });
    }
    let wall = d.assembly().next_instance();
    d.run(Command::MakeGroup {
        selection: Default::default(),
        instances: [chunk, chunk2].into(),
    });
    d.run(Command::NameInstance {
        instance: wall,
        name: "Wall".into(),
    });
    let post = d.assembly().next_instance();
    let sel = box_at(&mut d, 5.0, 0.0, 0.5);
    d.run(Command::MakeComponent {
        selection: sel,
        instances: Default::default(),
        name: "Post".into(),
    });
    let copies = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [post].into(),
        step: Change::Translate {
            by: DVec3::new(0.0, 2.0, 0.0),
        },
        count: 2,
    });
    // The last copy mirrored in x about its own middle.
    let last = prism_doc::InstanceId(copies.0 + 1);
    d.run(Command::Place {
        instances: [last].into(),
        change: Change::Scale {
            centre: DVec3::new(5.25, 4.25, 0.0),
            factor: DVec3::new(-1.0, 1.0, 1.0),
        },
    });
    box_at(&mut d, -3.0, 0.0, 1.0);
    d
}

fn glb() -> ExportOptions {
    ExportOptions {
        unit: prism_math::LengthUnit::Metres,
        ..ExportOptions::for_format(Format::Glb)
    }
}

fn mat(m: &[f64; 16]) -> DMat4 {
    DMat4::from_cols_array(m)
}

/// Every node's world matrix (parents' times its own).
fn worlds(s: &Scene) -> Vec<DMat4> {
    let mut out = vec![DMat4::IDENTITY; s.nodes.len()];
    let mut stack: Vec<(usize, DMat4)> = s.roots.iter().map(|r| (*r, DMat4::IDENTITY)).collect();
    while let Some((k, parent)) = stack.pop() {
        let w = parent * s.nodes[k].matrix.as_ref().map_or(DMat4::IDENTITY, mat);
        out[k] = w;
        for c in &s.nodes[k].children {
            stack.push((*c, w));
        }
    }
    out
}

fn by_name(s: &Scene, name: &str) -> Vec<usize> {
    (0..s.nodes.len())
        .filter(|k| s.nodes[*k].name == name)
        .collect()
}

#[test]
fn groups_and_components_become_named_nested_nodes() {
    let d = document();
    let s = structured(d.assembly(), &glb(), "Yard", true);
    let names: Vec<&str> = s.roots.iter().map(|r| s.nodes[*r].name.as_str()).collect();
    assert_eq!(names, ["Yard", "Wall", "Post", "Post 2", "Post 3"]);
    let wall = by_name(&s, "Wall")[0];
    let kids: Vec<&str> = s.nodes[wall]
        .children
        .iter()
        .map(|c| s.nodes[*c].name.as_str())
        .collect();
    assert_eq!(
        kids,
        ["Chunk", "Chunk 2"],
        "nested, and the second name made unique"
    );
    assert!(
        s.nodes[wall].mesh.is_none(),
        "the wall itself holds no faces"
    );
    // One shared mesh for the component.
    let posts: Vec<Option<usize>> = ["Post", "Post 2", "Post 3"]
        .iter()
        .map(|n| s.nodes[by_name(&s, n)[0]].mesh)
        .collect();
    assert!(posts[0].is_some() && posts.iter().all(|m| *m == posts[0]));
    assert_eq!(
        s.nodes[by_name(&s, "Post 2")[0]].extras,
        Some(serde_json::json!({"prism": {"kind": "component", "definition": "Post"}}))
    );
    assert_eq!(
        s.nodes[wall].extras,
        Some(serde_json::json!({"prism": {"kind": "group"}}))
    );
}

#[test]
fn every_vertex_lands_where_the_flattened_export_puts_it() {
    let d = document();
    let o = glb();
    let s = structured(d.assembly(), &o, "Yard", true);
    let w = worlds(&s);
    let mut placed: Vec<[f64; 3]> = Vec::new();
    let mut tris = 0;
    for (k, n) in s.nodes.iter().enumerate() {
        if let Some(m) = n.mesh {
            for p in &s.meshes[m].positions {
                placed.push(w[k].transform_point3(*p).to_array());
            }
            tris += s.meshes[m].triangle_count();
        }
    }
    let flat = to_painted_mesh(&d.assembly().flattened_visible(), None, &o, d.assembly());
    assert_eq!(tris, flat.triangle_count());
    let mut want: Vec<[f64; 3]> = flat.positions.iter().map(|p| p.to_array()).collect();
    let key = |p: &[f64; 3]| p.map(|c| (c * 1e6).round() as i64);
    placed.sort_by_key(key);
    want.sort_by_key(key);
    assert_eq!(placed.len(), want.len());
    for (a, b) in placed.iter().zip(&want) {
        assert!(
            DVec3::from_array(*a).distance(DVec3::from_array(*b)) < 1e-9,
            "{a:?} {b:?}"
        );
    }
}

#[test]
fn a_groups_origin_is_its_nodes_translation() {
    let d = document();
    let s = structured(d.assembly(), &glb(), "Yard", true);
    // Chunk 2's box runs from x = 1; its group's origin is that corner.
    let k = by_name(&s, "Chunk 2")[0];
    let world = worlds(&s)[k];
    // Prism (1, 0, 0) is glTF (1, 0, -0), Y up.
    assert!(
        world
            .transform_point3(DVec3::ZERO)
            .distance(DVec3::new(1.0, 0.0, 0.0))
            < 1e-12
    );
    let m = &s.meshes[s.nodes[k].mesh.unwrap()];
    let lo = m
        .positions
        .iter()
        .fold(DVec3::splat(f64::MAX), |a, p| a.min(*p));
    assert!(
        lo.distance(DVec3::new(0.0, 0.0, -1.0)) < 1e-12,
        "local, from its own corner: {lo}"
    );
}

#[test]
fn a_mirrored_copy_keeps_its_faces_facing_out() {
    let d = document();
    let s = structured(d.assembly(), &glb(), "Yard", true);
    let w = worlds(&s);
    for name in ["Post", "Post 3"] {
        let k = by_name(&s, name)[0];
        let mirrored = w[k].determinant() < 0.0;
        assert_eq!(mirrored, name == "Post 3");
        let m = &s.meshes[s.nodes[k].mesh.unwrap()];
        let pts: Vec<DVec3> = m
            .positions
            .iter()
            .map(|p| w[k].transform_point3(*p))
            .collect();
        let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        for p in &m.polygons {
            let (a, b, c) = (pts[p[0] as usize], pts[p[1] as usize], pts[p[2] as usize]);
            // glTF turns the winding round under a mirroring transform.
            let mut n = (b - a).cross(c - a);
            if mirrored {
                n = -n;
            }
            assert!(
                n.dot((a + b + c) / 3.0 - centre) > 0.0,
                "{name}: a face points in"
            );
        }
    }
}

#[test]
fn two_exports_are_the_same() {
    let d = document();
    let a = prism_file::mesh::export_structured(d.assembly(), &glb(), "Yard", true);
    let b = prism_file::mesh::export_structured(d.assembly(), &glb(), "Yard", true);
    assert_eq!(a, b);
    // The JSON names every node.
    let len = u32::from_le_bytes(a[12..16].try_into().unwrap()) as usize;
    let doc: serde_json::Value = serde_json::from_slice(&a[20..20 + len]).unwrap();
    let names: Vec<&str> = doc["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"Chunk 2") && names.contains(&"Post 3"),
        "{names:?}"
    );
    assert_eq!(
        doc["meshes"].as_array().unwrap().len(),
        4,
        "yard, two chunks, one post"
    );
}

#[test]
fn merged_export_is_the_same_as_before() {
    let b = std::fs::read(format!(
        "{}/../../docs/acceptance/files/textured.prism",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let c = prism_file::read(&b).unwrap();
    let o = ExportOptions::for_format(Format::Glb);
    let m = c.assembly.flattened_visible();
    let now = prism_file::mesh::export_with(&m, None, &o, Some(&c.assembly));
    assert_eq!(
        now,
        include_bytes!("data/merged-textured.glb"),
        "byte for byte"
    );
}

#[test]
fn hidden_groups_are_left_out() {
    let mut d = document();
    let wall = d
        .assembly()
        .root()
        .instances
        .keys()
        .next()
        .copied()
        .unwrap();
    d.run(Command::SetHidden {
        instances: [wall].into(),
        hidden: true,
    });
    let s = structured(d.assembly(), &glb(), "Yard", true);
    assert!(by_name(&s, "Wall").is_empty());
    let s = structured(d.assembly(), &glb(), "Yard", false);
    assert_eq!(by_name(&s, "Wall").len(), 1);
}
