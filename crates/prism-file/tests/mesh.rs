//! Mesh import and export (brief 09).

use prism_file::mesh::{self, ExportOptions, Format, Mesh};
use prism_math::{DVec3, LengthUnit};
use prism_topo::Model;

fn areas(m: &Model) -> Vec<f64> {
    let mut a: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id)).collect();
    a.sort_by(f64::total_cmp);
    a
}

fn close(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() <= tol * (1.0 + y.abs()), "{a:?} vs {b:?}");
    }
}

fn ok(m: &Model) {
    let p = prism_ops::check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn models() -> Vec<Model> {
    vec![
        prism_tools::scenes::cube().model().clone(),
        // A box with a pocket: faces with holes.
        prism_tools::scenes::pocket().model().clone(),
    ]
}

/// Out and back in: the same faces (after merging), the same areas.
#[test]
fn every_format_round_trips() {
    for m in models() {
        for f in Format::ALL {
            let o = ExportOptions::for_format(f);
            let bytes = mesh::export(&m, None, &o);
            assert_eq!(
                bytes,
                mesh::export(&m, None, &o),
                "{f:?}: the same bytes twice"
            );
            let back = mesh::read(&bytes, f).unwrap_or_else(|e| panic!("{f:?}: {e}"));
            let scale = mesh::stated_unit(&bytes, f).unwrap_or(o.unit.metres());
            let model = mesh::build(&back, scale, o.y_up, true);
            ok(&model);
            // f32 formats (STL, GLB) round to about 1e-7.
            close(&areas(&model), &areas(&m), 1e-5);
            assert_eq!(model.face_count(), m.face_count(), "{f:?}");
        }
    }
}

#[test]
fn without_merging_triangles_stay_triangles() {
    let m = prism_tools::scenes::cube().model().clone();
    let o = ExportOptions::for_format(Format::Stl);
    let back = mesh::read(&mesh::export(&m, None, &o), Format::Stl).unwrap();
    assert_eq!(back.triangle_count(), 12);
    let model = mesh::build(&back, 1e-3, false, false);
    assert_eq!(model.face_count(), 12);
    ok(&model);
}

#[test]
fn hand_written_files_read() {
    let stl = b"solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n";
    let m = mesh::read(stl, Format::Stl).unwrap();
    assert_eq!((m.positions.len(), m.polygons.len()), (3, 1));

    let obj = b"# a quad, a pentagon, negative indices\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1 2/2 3 4\nv 3 0 0\nv 4 0 0\nv 4.5 1 0\nv 3.5 2 0\nv 2.5 1 0\nf -5 -4 -3 -2 -1\n";
    let m = mesh::read(obj, Format::Obj).unwrap();
    assert_eq!(m.polygons, vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7, 8]]);
    let model = mesh::build(&m, 1.0, false, true);
    assert_eq!(model.face_count(), 2);

    let ply = b"ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
    let m = mesh::read(ply, Format::Ply).unwrap();
    assert_eq!(m.polygons, vec![vec![0, 1, 2, 3]]);
    assert_eq!(m.positions[2], DVec3::new(1.0, 1.0, 0.0));
}

#[test]
fn glb_applies_node_transforms() {
    // A unit triangle in a node moved by (10, 0, 0) and scaled by 2.
    let src = Mesh {
        positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y],
        polygons: vec![vec![0, 1, 2]],
    };
    let m = {
        let mut model = Model::new();
        let tri = mesh::build(&src, 1.0, false, false);
        model.append(&tri);
        model
    };
    let o = ExportOptions {
        format: Format::Glb,
        unit: LengthUnit::Metres,
        y_up: false,
    };
    let bytes = mesh::export(&m, None, &o);
    // Put a transform on the node.
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut doc: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
    doc["nodes"][0]["translation"] = serde_json::json!([10.0, 0.0, 0.0]);
    doc["nodes"][0]["scale"] = serde_json::json!([2.0, 2.0, 2.0]);
    let mut js = serde_json::to_vec(&doc).unwrap();
    while !js.len().is_multiple_of(4) {
        js.push(b' ');
    }
    let bin = &bytes[20 + json_len..];
    let mut glb = Vec::new();
    let total = 20 + js.len() + bin.len();
    for w in [
        0x4654_6C67u32,
        2,
        total as u32,
        js.len() as u32,
        0x4E4F_534A,
    ] {
        glb.extend_from_slice(&w.to_le_bytes());
    }
    glb.extend_from_slice(&js);
    glb.extend_from_slice(bin);
    let back = mesh::read(&glb, Format::Glb).unwrap();
    let xs: Vec<f64> = back.positions.iter().map(|p| p.x).collect();
    assert!(xs.contains(&10.0) && xs.contains(&12.0), "{xs:?}");
}

#[test]
fn coplanar_triangles_merge_and_rings_get_holes() {
    // Two triangles of a square: one face.
    let sq = Mesh {
        positions: vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ],
        polygons: vec![vec![0, 1, 2], vec![0, 2, 3]],
    };
    let m = mesh::build(&sq, 1.0, false, true);
    assert_eq!(m.face_count(), 1);
    assert_eq!(m.edge_count(), 4, "the diagonal is gone");
    // A ring: a 3x3 square with the middle cell missing, as 8 quads.
    let mut ring = Mesh::default();
    for y in 0..4 {
        for x in 0..4 {
            ring.positions.push(DVec3::new(x as f64, y as f64, 0.0));
        }
    }
    for y in 0..3 {
        for x in 0..3 {
            if (x, y) == (1, 1) {
                continue;
            }
            let i = |x: u32, y: u32| y * 4 + x;
            ring.polygons
                .push(vec![i(x, y), i(x + 1, y), i(x + 1, y + 1), i(x, y + 1)]);
        }
    }
    let m = mesh::build(&ring, 1.0, false, true);
    assert_eq!(m.face_count(), 1);
    let f = m.faces().next().unwrap().1;
    assert_eq!(f.holes.len(), 1);
    close(&areas(&m), &[8.0], 1e-12);
    ok(&m);
    // A bend: two triangles not on one plane stay apart.
    let bend = Mesh {
        positions: vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
        ],
        polygons: vec![vec![0, 1, 2], vec![1, 0, 3]],
    };
    assert_eq!(mesh::build(&bend, 1.0, false, true).face_count(), 2);
}

#[test]
fn the_selection_exports_alone() {
    let m = prism_tools::scenes::cube().model().clone();
    let one: std::collections::BTreeSet<_> = m.faces().map(|(id, _)| id).take(1).collect();
    let o = ExportOptions::for_format(Format::Stl);
    let back = mesh::read(&mesh::export(&m, Some(&one), &o), Format::Stl).unwrap();
    assert_eq!(back.triangle_count(), 2);
}

#[test]
fn units_and_up_axis() {
    let m = prism_tools::scenes::cube().model().clone();
    let (lo, hi) = m.bbox().unwrap();
    let o = ExportOptions::for_format(Format::Obj);
    assert!(o.y_up);
    let back = mesh::read(&mesh::export(&m, None, &o), Format::Obj).unwrap();
    // In millimetres, Y up: the cube's height is along y.
    let size = back.size();
    assert!((size.y - (hi.z - lo.z) * 1000.0).abs() < 1e-6, "{size:?}");
    let model = mesh::build(&back, 1e-3, true, true);
    let (lo2, hi2) = model.bbox().unwrap();
    assert!(lo2.distance(lo) < 1e-9 && hi2.distance(hi) < 1e-9);
}

#[cfg(not(target_arch = "wasm32"))]
mod fuzz {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

        /// Damaged and cut-short files never panic.
        #[test]
        fn damaged_mesh_files_never_panic(
            which in 0usize..5,
            flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..6),
            cut in any::<prop::sample::Index>(),
        ) {
            let f = Format::ALL[which];
            let m = prism_tools::scenes::pocket().model().clone();
            let mut bytes = mesh::export(&m, None, &ExportOptions::for_format(f));
            for (i, x) in flips {
                let k = i.index(bytes.len());
                bytes[k] ^= x | 1;
            }
            let n = cut.index(bytes.len() + 1);
            if let Ok(mesh) = mesh::read(&bytes[..n], f) {
                let _ = mesh::build(&mesh, 1e-3, false, true);
            }
        }
    }
}

/// Prints how long the Large Scene takes out and back in (NOTES.md).
#[test]
#[ignore = "prints timings"]
fn large_scene_timing() {
    let m = prism_tools::scenes::large(50).model().clone();
    for f in Format::ALL {
        let o = ExportOptions::for_format(f);
        let t = std::time::Instant::now();
        let bytes = mesh::export(&m, None, &o);
        let out = t.elapsed();
        let t = std::time::Instant::now();
        let back = mesh::read(&bytes, f).unwrap();
        let model = mesh::build(
            &back,
            mesh::stated_unit(&bytes, f).unwrap_or(o.unit.metres()),
            o.y_up,
            true,
        );
        println!(
            "{:10} {:>8} KB  export {:>6.0} ms  import {:>6.0} ms  {} faces -> {}",
            f.name(),
            bytes.len() / 1024,
            out.as_secs_f64() * 1e3,
            t.elapsed().as_secs_f64() * 1e3,
            m.face_count(),
            model.face_count()
        );
    }
}
