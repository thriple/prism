//! Crossings with freeform faces (Solids & CAD part 3, step 3b): traced in
//! the patch's (u, v) and kept as a cubic on both surfaces; solids with
//! freeform faces checked by the kernel (step 3c).

use std::sync::Arc;

use prism_geom::{Curve, NurbsSurface, Patch, Spline, Surface};
use prism_math::DVec3;
use prism_ops::crossing::{Shape, crossings_with};
use prism_topo::{Face, FaceId, Model, VertexId};

/// A bicubic bump over the unit square, 0.4 high in the middle.
fn bump() -> NurbsSurface {
    let mut pts = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let mid = (i == 1 || i == 2) && (j == 1 || j == 2);
            pts.push(DVec3::new(
                i as f64 / 3.0,
                j as f64 / 3.0,
                if mid { 0.4 } else { 0.05 * i as f64 },
            ));
        }
    }
    let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    NurbsSurface::new(
        [3, 3],
        [k.clone(), k],
        [4, 4],
        pts,
        vec![1.0; 16],
        [false, false],
    )
    .unwrap()
}

/// The whole bump as a face, its sides iso lines.
fn bump_face(m: &mut Model) -> FaceId {
    let s = Arc::new(bump());
    let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let vs: Vec<VertexId> = uv
        .iter()
        .map(|(u, v)| m.add_vertex(s.eval(*u, *v)))
        .collect();
    let mut edges = Vec::new();
    for i in 0..4 {
        let (a, b) = (uv[i], uv[(i + 1) % 4]);
        let (d, at, t0, t1) = if a.1 == b.1 {
            (0, a.1, a.0, b.0)
        } else {
            (1, a.0, a.1, b.1)
        };
        let spline = Spline {
            curve: Arc::new(s.iso(d, at).unwrap()),
            t0,
            t1,
        };
        edges.push(
            m.add_curve_edge(vs[i], vs[(i + 1) % 4], Curve::Spline(spline), None)
                .unwrap(),
        );
    }
    m.add_face(Face::with_edges(
        vs,
        vec![],
        Surface::Nurbs(Patch::whole(Arc::try_unwrap(s).unwrap())),
        vec![edges],
    ))
}

/// A wall at x = 0.37, wider and taller than the bump.
fn wall(m: &mut Model) -> FaceId {
    let before: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    prism_ops::add_polygon(
        m,
        &[
            DVec3::new(0.37, -0.5, -1.0),
            DVec3::new(0.37, 1.5, -1.0),
            DVec3::new(0.37, 1.5, 1.0),
            DVec3::new(0.37, -0.5, 1.0),
        ],
    );
    m.faces()
        .map(|(id, _)| id)
        .find(|id| !before.contains(id))
        .expect("the wall")
}

#[test]
fn a_plane_through_a_bump_is_one_cubic_on_both() {
    let mut m = Model::new();
    let f = bump_face(&mut m);
    let w = wall(&mut m);
    let tol = m.tol();
    let pieces = crossings_with(&m, f, w);
    assert_eq!(pieces.len(), 1, "{pieces:?}");
    let Shape::Spline(c) = &pieces[0].shape else {
        panic!("not a cubic: {:?}", pieces[0].shape);
    };
    let surf = m.face(f).unwrap().surface.clone();
    let (d0, d1) = c.domain();
    for i in 0..=200 {
        let p = c.eval(d0 + (d1 - d0) * i as f64 / 200.0);
        assert!((p.x - 0.37).abs() <= tol.dist, "{p}");
        assert!(surf.distance(p).abs() <= tol.dist, "{p}");
    }
    // From the side at y = 0 to the side at y = 1.
    let (a, b) = (pieces[0].start(), pieces[0].end());
    let ys = [a.y.min(b.y), a.y.max(b.y)];
    assert!(
        ys[0].abs() <= tol.dist && (ys[1] - 1.0).abs() <= tol.dist,
        "{a} {b}"
    );
}

#[test]
fn intersect_faces_cuts_the_bump_in_two() {
    let mut m = Model::new();
    let f = bump_face(&mut m);
    wall(&mut m);
    let area = m.face_area(f);
    assert!(prism_ops::intersect::intersect_faces(&mut m, None) >= 1);
    let bumps: Vec<FaceId> = m
        .faces()
        .filter(|(_, x)| x.surface.patch().is_some())
        .map(|(id, _)| id)
        .collect();
    assert_eq!(bumps.len(), 2);
    let sum: f64 = bumps.iter().map(|g| m.face_area(*g)).sum();
    assert!((sum - area).abs() < 1e-9 * area, "{sum} vs {area}");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
}

#[test]
fn two_bumps_cross_in_a_closed_loop_on_both() {
    let mut m = Model::new();
    let f = bump_face(&mut m);
    // A second bump turned over: above the first at its sides, below it in
    // the middle. Moved along x, so the crossing is not flat (a flat loop would
    // also close a flat face, as drawing one does).
    let mut n = Model::new();
    bump_face(&mut n);
    prism_ops::transform::affine(
        &mut n,
        &prism_math::DAffine3::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.1, 0.0, 0.35,
        ]),
    );
    let g = m.append(&n)[0];
    let tol = m.tol();
    let pieces = crossings_with(&m, f, g);
    assert!(!pieces.is_empty());
    let (sf, sg) = (
        m.face(f).unwrap().surface.clone(),
        m.face(g).unwrap().surface.clone(),
    );
    for p in &pieces {
        let Shape::Spline(c) = &p.shape else {
            panic!("not a cubic: {:?}", p.shape);
        };
        let (d0, d1) = c.domain();
        for i in 0..=100 {
            let x = c.eval(d0 + (d1 - d0) * i as f64 / 100.0);
            assert!(sf.distance(x).abs() <= tol.dist, "{x}");
            assert!(sg.distance(x).abs() <= tol.dist, "{x}");
        }
    }
    // The pieces close up.
    let ends: Vec<DVec3> = pieces.iter().flat_map(|p| [p.start(), p.end()]).collect();
    for e in &ends {
        let n = ends.iter().filter(|x| x.distance(*e) <= tol.dist).count();
        assert!(n >= 2, "an open end at {e}");
    }
    assert_eq!(
        prism_ops::intersect::intersect_faces(&mut m, None),
        pieces.len()
    );
    assert_eq!(m.faces().count(), 4, "each bump cut into a disc and a ring");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
}

#[test]
fn a_mirrored_freeform_face_still_checks_clean() {
    let mut n = Model::new();
    bump_face(&mut n);
    prism_ops::transform::affine(
        &mut n,
        &prism_math::DAffine3::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.1, 0.0, 0.35,
        ]),
    );
    assert_eq!(prism_ops::check::problems(&n), Vec::<String>::new());
}

/// The edge between two vertices: the one there, or a new line.
fn edge(m: &mut Model, a: VertexId, b: VertexId) -> prism_topo::EdgeId {
    let found = m
        .edges()
        .find(|(_, e)| e.has(a) && e.has(b))
        .map(|(id, _)| id);
    found.unwrap_or_else(|| m.add_edge(a, b).unwrap())
}

/// A block from z = -1 up to the bump: four flat walls, a flat base and
/// the bump on top. Its volume is 1 + the bump's mean height, the mean of
/// its control points' heights (2.5 / 16).
fn bump_block() -> Model {
    let mut m = Model::new();
    let top = bump_face(&mut m);
    let c = m.face(top).unwrap().outer.clone();
    let b: Vec<VertexId> = c
        .iter()
        .map(|v| {
            let p = m.vertex(*v);
            m.add_vertex(DVec3::new(p.x, p.y, -1.0))
        })
        .collect();
    let flat = |m: &mut Model, vs: Vec<VertexId>| {
        let pts: Vec<DVec3> = vs.iter().map(|v| m.vertex(*v)).collect();
        let n = prism_math::newell(&pts).normalize();
        let edges: Vec<_> = (0..vs.len())
            .map(|i| edge(m, vs[i], vs[(i + 1) % vs.len()]))
            .collect();
        m.add_face(Face::with_edges(
            vs,
            vec![],
            Surface::Plane(prism_math::Plane::new(pts[0], n)),
            vec![edges],
        ));
    };
    for i in 0..4 {
        let j = (i + 1) % 4;
        // Seen from outside: along the base, up, back along the top.
        flat(&mut m, vec![b[i], b[j], c[j], c[i]]);
    }
    flat(&mut m, b.iter().rev().copied().collect());
    m
}

const BLOCK: f64 = 1.0 + 2.5 / 16.0;

/// The bump's height integrated over a square of it (Gauss-Legendre with
/// three points is exact for the bicubic).
fn under_bump(lo: f64, hi: f64) -> f64 {
    let s = bump();
    let (x, w) = (
        [-(0.6f64).sqrt(), 0.0, (0.6f64).sqrt()],
        [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0],
    );
    let h = (hi - lo) / 2.0;
    let mut sum = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            let (u, v) = (lo + h * (1.0 + x[i]), lo + h * (1.0 + x[j]));
            sum += w[i] * w[j] * s.eval(u, v).z;
        }
    }
    sum * h * h
}

/// The volume the kernel gives for the one solid in `m`, once its checker
/// has passed it.
fn kernel_volume(m: &Model) -> f64 {
    let all = prism_ops::validate::validate(m);
    assert_eq!(all.len(), 1);
    assert!(all[0].valid, "{all:?}");
    assert_eq!(all[0].kernel_skipped, None, "checked by the kernel");
    let r = prism_ops::bridge::region(m, &all[0].faces).unwrap();
    prism_kernel::solid::Solid::build(&r, m.tol().dist)
        .and_then(|s| s.volume())
        .unwrap()
}

#[test]
fn the_bump_block_is_a_solid_of_its_volume() {
    let m = bump_block();
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let all = prism_ops::validate::validate(&m);
    let v = all[0].volume.unwrap();
    assert!((v - BLOCK).abs() < 1e-9, "{v} vs {BLOCK}");
    let k = kernel_volume(&m);
    assert!((k - BLOCK).abs() < 1e-9, "the kernel's {k} vs {BLOCK}");
}

#[test]
fn a_box_subtracted_from_the_bump_block() {
    let a = bump_block();
    let mut b = Model::new();
    prism_ops::add_rect(&mut b, 0.25, 0.25, 0.75, 0.75, 0.0);
    let f = prism_ops::face_at(&b, DVec3::new(0.5, 0.5, 0.0)).unwrap();
    let up = b.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut b, f, 2.0 * up, false);
    let out = prism_ops::boolean::boolean(&a, &b, prism_ops::boolean::Op::Subtract).unwrap();
    assert_eq!(prism_ops::check::problems(&out), Vec::<String>::new());
    let all = prism_ops::validate::validate(&out);
    assert!(all.iter().all(|c| c.valid), "{all:?}");
    let v: f64 = all.iter().map(|c| c.volume.unwrap()).sum();
    let want = BLOCK - under_bump(0.25, 0.75);
    // The kernel's checker passes it, and its volume agrees.
    let k = kernel_volume(&out);
    assert!(
        (k - want).abs() < out.tol().dist * 0.1,
        "the kernel's {k} vs {want}"
    );
    // The cut's edges are cubics within the tolerance of the crossing, so
    // the volume is as close as a tenth of the tolerance.
    assert!((v - want).abs() < out.tol().dist * 0.1, "{v} vs {want}");
}

#[test]
fn the_bump_blocks_top_pulls_out_by_the_distance() {
    let mut m = bump_block();
    let top = m
        .faces()
        .find(|(_, f)| f.surface.patch().is_some())
        .unwrap()
        .0;
    let old = m.face(top).unwrap().surface.clone();
    let area = m.face_area(top);
    assert_eq!(prism_ops::pushpull::refusal(&m, top, 0.1), None);
    prism_ops::push_pull(&mut m, top, 0.1, false);
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let all = prism_ops::validate::validate(&m);
    assert_eq!(all.len(), 1, "{all:?}");
    assert!(all[0].valid, "{all:?}");
    // Its curved sides meet the new top along many pieces: too slow for
    // the kernel's check yet.
    assert_eq!(
        all[0].kernel_skipped,
        Some(prism_ops::validate::KERNEL_TOO_LONG)
    );
    // About the top's area times 0.1 more.
    let grown = all[0].volume.unwrap() - BLOCK;
    assert!(
        (grown - area * 0.1).abs() < area * 0.1 * 0.1,
        "{grown} vs {area}"
    );
    // The new top is 0.1 out from the old one everywhere.
    let tol = m.tol();
    let new = m
        .faces()
        .filter_map(|(_, f)| f.surface.patch().map(|p| (f, p.clone())))
        .find(|(f, _)| (old.distance(m.vertex(f.outer[0])) - 0.1).abs() <= tol.dist)
        .expect("the moved top")
        .1;
    for i in 1..8 {
        for j in 1..8 {
            let p = new.surface.eval(i as f64 / 8.0, j as f64 / 8.0);
            assert!((old.distance(p) - 0.1).abs() <= tol.dist, "{p}");
        }
    }
}
