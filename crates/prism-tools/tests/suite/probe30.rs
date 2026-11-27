use prism_tools::session::{Recording, Session};

fn at(r: &Recording, n: usize) -> Session {
    let mut q = r.clone();
    q.events.truncate(n);
    Session::replay(&q)
}

#[test]
#[ignore]
fn probe30() {
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    let s = at(&r, 322);
    let m = s.work_model();
    let ef = m.edge_faces();
    let sel = s.selection();
    println!(
        "faces {} edges {} approx {} sel {:?}",
        m.faces().count(),
        m.edges().count(),
        m.edges().filter(|(_, e)| e.approx).count(),
        sel.edges
    );
    for e in &sel.edges {
        let x = m.edge(*e).unwrap();
        println!(
            "SEL {e:?} {:.4?}-{:.4?} {:?} faces {:?}",
            m.vertex(x.a),
            m.vertex(x.b),
            x.curve,
            ef.get(e)
        );
    }
    let mut kinds = std::collections::BTreeMap::new();
    for (id, e) in m.edges().filter(|(_, e)| e.approx) {
        let fs = ef.get(&id).cloned().unwrap_or_default();
        let k: Vec<String> = fs
            .iter()
            .map(|f| {
                let x = m.face(*f).unwrap();
                format!("{:?}", x.surface).chars().take(9).collect()
            })
            .collect();
        *kinds
            .entry(format!("{k:?} cid {:?}", e.curve_id))
            .or_insert(0) += 1;
        let _ = id;
    }
    for (k, n) in kinds {
        println!("APPROX {n} x {k}");
    }
}

#[test]
#[ignore]
fn probe30_round() {
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    for n in [0, 96, 160, 180, 300] {
        let s0 = at(&r, n);
        println!(
            "at {n}: faces {} approx {}",
            s0.work_model().faces().count(),
            s0.work_model().edges().filter(|(_, e)| e.approx).count()
        );
    }
    let s = at(&r, 322);
    let m = s.work_model().clone();
    let h = prism_ops::health::check(&m);
    println!(
        "health watertight {} leaky {} nm {} rev {}",
        h.watertight,
        h.leaky,
        h.non_manifold.edges.len(),
        h.reversed.faces.len()
    );
    let sel = s.selection().edges.clone();
    let vol: f64 = m
        .faces()
        .map(|(id, _)| prism_ops::health::face_volume(&m, id))
        .sum();
    println!("signed volume {vol}");
    for e in &sel {
        println!("concave? {e:?}");
    }
    let mut w = m.clone();
    let ids: Vec<_> = w.edges().map(|(id, _)| id).collect();
    for id in ids {
        w.set_approx(id, false);
    }
    for size in [0.05, 0.02] {
        let t = std::time::Instant::now();
        let mut x = w.clone();
        let got = prism_ops::rounds::round_edges(&mut x, &sel, size, true);
        println!(
            "cleared {size}: {:?} in {:?}",
            got.map(|f| f.len()),
            t.elapsed()
        );
        let mut y = m.clone();
        let got = prism_ops::rounds::round_edges(&mut y, &sel, size, true);
        println!(
            "as is {size}: {:?} approx {} -> {} faces {} -> {} problems {:?}",
            got.map(|f| f.len()),
            m.edges().filter(|(_, e)| e.approx).count(),
            y.edges().filter(|(_, e)| e.approx).count(),
            m.faces().count(),
            y.faces().count(),
            prism_ops::check::problems(&y).len()
        );
        let (a, b) = (prism_ops::health::check(&m), prism_ops::health::check(&y));
        let v2: f64 = y
            .faces()
            .map(|(id, _)| prism_ops::health::face_volume(&y, id))
            .sum();
        println!("   dv {}", v2 - vol);
        println!(
            "   nm {} -> {} leaky {} -> {} rev {} -> {}",
            a.non_manifold.edges.len(),
            b.non_manifold.edges.len(),
            a.leaky,
            b.leaky,
            a.reversed.faces.len(),
            b.reversed.faces.len()
        );
        let new: Vec<_> = y
            .faces()
            .map(|(id, _)| id)
            .filter(|id| m.face(*id).is_none())
            .collect();
        println!(
            "   new {:?} reversed has new: {:?}; reversed sample {:?}; watertight {} -> {}",
            new,
            new.iter()
                .map(|f| b.reversed.faces.contains(f))
                .collect::<Vec<_>>(),
            b.reversed.faces.iter().take(5).collect::<Vec<_>>(),
            a.watertight,
            b.watertight
        );
    }
}

#[test]
#[ignore]
fn probe30_steps() {
    use prism_tools::session::Input;
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    let mut q = r.clone();
    q.events.clear();
    let mut s = Session::replay(&q);
    let (lo, hi): (usize, usize) = std::env::var("RANGE")
        .ok()
        .and_then(|v| {
            let mut it = v.split('-').map(|x| x.parse().unwrap());
            Some((it.next()?, it.next()?))
        })
        .unwrap_or((0, 3037));
    let mut last = String::new();
    for (i, e) in r.events.iter().enumerate() {
        s.handle(e);
        if i < lo || i > hi {
            continue;
        }
        let key = matches!(
            e.input,
            Input::Press { .. }
                | Input::Release { .. }
                | Input::Tool { .. }
                | Input::Action { .. }
                | Input::Round { .. }
        );
        let m = s.work_model();
        let line = format!(
            "tool {} notice {} hint {:?} sel e{} f{} g{} faces {} approx {}",
            s.tool(),
            s.hint_is_notice(),
            s.hint(),
            s.selection().edges.len(),
            s.selection().faces.len(),
            s.selected_groups().len(),
            m.faces().count(),
            m.edges().filter(|(_, e)| e.approx).count()
        );
        if key || line != last {
            println!("{i}: {line}");
            last = line;
        }
    }
}

#[test]
#[ignore]
fn probe30_sides() {
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    let s = at(&r, 322);
    let m = s.work_model().clone();
    let ef = m.edge_faces();
    for e in s.selection().edges.iter() {
        let x = m.edge(*e).unwrap();
        let mid = (m.vertex(x.a) + m.vertex(x.b)) * 0.5;
        for f in &ef[e] {
            let n = m.face(*f).unwrap().plane().unwrap().normal;
            let face = m.face(*f).unwrap();
            let c: prism_math::DVec3 = face
                .outer
                .iter()
                .map(|v| m.vertex(*v))
                .sum::<prism_math::DVec3>()
                / face.outer.len() as f64;
            // A point just off the face's middle, along its normal and against it.
            let probe = |p: prism_math::DVec3| prism_ops::face_at(&m, p).is_some();
            println!(
                "edge {e:?} face {f:?} normal {:.3?} centre {:.3?} mid {:.3?} verts {}",
                n,
                c,
                mid,
                face.outer.len()
            );
            let _ = probe;
        }
    }
    let h = prism_ops::health::check(&m);
    println!("interior before {:?}", h.interior.faces);
    for f in &h.interior.faces {
        let face = m.face(*f).unwrap();
        let pts: Vec<prism_math::DVec3> = face.outer.iter().map(|v| m.vertex(*v)).collect();
        let lo = pts
            .iter()
            .fold(prism_math::DVec3::splat(f64::INFINITY), |a, b| a.min(*b));
        let hi = pts
            .iter()
            .fold(prism_math::DVec3::splat(f64::NEG_INFINITY), |a, b| {
                a.max(*b)
            });
        println!(
            "interior {f:?} plane {:?} box {:.3?}..{:.3?} verts {}",
            face.plane().map(|p| p.normal),
            lo,
            hi,
            pts.len()
        );
    }
    // Faces near the corner of edge 499, other than its own.
    let x = m.edge(prism_topo::EdgeId(499)).unwrap();
    let mid = (m.vertex(x.a) + m.vertex(x.b)) * 0.5;
    let mut near: Vec<(f64, prism_topo::FaceId)> = m
        .faces()
        .filter_map(|(id, f)| {
            let d = f
                .outer
                .iter()
                .map(|v| m.vertex(*v).distance(mid))
                .fold(f64::INFINITY, f64::min);
            (d < 0.08).then_some((d, id))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (d, f) in near.iter().take(12) {
        println!(
            "near {f:?} d{d:.3} n {:.3?}",
            m.face(*f).unwrap().plane().map(|p| p.normal)
        );
    }
}

#[test]
#[ignore]
fn probe30_arc() {
    let r: Recording = serde_json::from_str(include_str!("../data/owner-session-30.json")).unwrap();
    let a = at(&r, 1737);
    let b = at(&r, 1741);
    let (ma, mb) = (a.work_model(), b.work_model());
    let new: Vec<_> = mb.edges().filter(|(id, _)| ma.edge(*id).is_none()).collect();
    println!("new edges {} gone {}", new.len(), ma.edges().filter(|(id, _)| mb.edge(*id).is_none()).count());
    for (id, e) in new.iter().take(40) {
        println!("  {id:?} {:.4?}-{:.4?} {} approx {} cid {:?} soft {}", mb.vertex(e.a), mb.vertex(e.b), if e.curve.is_curved() { "arc" } else { "line" }, e.approx, e.curve_id, e.soft);
    }
    let old_c: std::collections::BTreeSet<_> = ma.edges().filter_map(|(_, e)| e.curve_id).collect();
    let fresh: Vec<_> = mb.edges().filter(|(_, e)| e.curve.is_curved() || e.curve_id.is_some_and(|c| !old_c.contains(&c))).filter(|(id, _)| ma.edge(*id).is_none()).collect();
    println!("fresh curve edges {}", fresh.len());
    for (id, e) in fresh.iter().take(10) { println!("  F {id:?} {:.4?}-{:.4?} {:?} approx {} cid {:?}", mb.vertex(e.a), mb.vertex(e.b), e.curve, e.approx, e.curve_id); }
    let ef = mb.edge_faces();
    if let Some((id, _)) = new.first() { println!("faces of first {:?}", ef.get(id)); }
    println!("hint after {:?} notice {}", b.hint(), b.hint_is_notice());
    let cmd = b.document().log().last().cloned();
    println!("CMD {}", format!("{cmd:?}").chars().take(600).collect::<String>());
    let t = a.open_transform();
    println!("open {:?} transform cols {:.4?} {:.4?} {:.4?} lens {:.4} {:.4} {:.4}", a.open_path(), t.matrix3.x_axis, t.matrix3.y_axis, t.matrix3.z_axis, t.matrix3.x_axis.length(), t.matrix3.y_axis.length(), t.matrix3.z_axis.length());
}
