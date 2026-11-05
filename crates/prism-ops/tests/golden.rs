//! The golden cases from the browser prototype
//! (`docs/acceptance/golden-cases.json`), run at three scales.

use std::collections::BTreeSet;

use prism_geom::Surface;
use prism_math::{DVec3, Plane, newell};
use prism_ops::select::Selection;
use prism_topo::{Face, Model, VertexId};
use serde_json::Value;

const SCALES: [f64; 3] = [1e-4, 1.0, 1e3];

pub fn cases() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/acceptance/golden-cases.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("golden cases file"))
        .expect("valid JSON")
}

fn v3(v: &Value, k: f64) -> DVec3 {
    let a = v.as_array().expect("point");
    DVec3::new(
        a[0].as_f64().expect("x") * k,
        a[1].as_f64().expect("y") * k,
        a[2].as_f64().expect("z") * k,
    )
}

fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or_else(|| panic!("missing {key}"))
}

/// The prototype's starting cube: x and y from -1 to 1, z from 1 to 3.
pub fn cube(k: f64) -> Model {
    let mut m = Model::new();
    let c: Vec<VertexId> = [
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
        [-1.0, -1.0, 3.0],
        [1.0, -1.0, 3.0],
        [1.0, 1.0, 3.0],
        [-1.0, 1.0, 3.0],
    ]
    .iter()
    .map(|p| m.add_vertex(DVec3::from_array(*p) * k))
    .collect();
    for l in [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ] {
        let outer: Vec<VertexId> = l.iter().map(|i| c[*i]).collect();
        for i in 0..4 {
            m.add_edge(outer[i], outer[(i + 1) % 4]);
        }
        let pts = m.points(&outer);
        let n = newell(&pts);
        let centre = pts.iter().copied().sum::<DVec3>() / 4.0;
        m.add_face(Face {
            outer,
            holes: vec![],
            surface: Surface::Plane(Plane::new(centre, n)),
        });
    }
    m
}

/// Run one case's operations at scale `k`.
pub fn run(case: &Value, k: f64) -> Model {
    let mut m = if case["start"] == "cube" {
        cube(k)
    } else {
        Model::new()
    };
    for op in case["ops"].as_array().expect("ops") {
        match op["op"].as_str().expect("op name") {
            "segment" => {
                prism_ops::add_segment(&mut m, v3(&op["p"], k), v3(&op["q"], k));
            }
            "rect" => {
                prism_ops::add_rect(
                    &mut m,
                    num(op, "x0") * k,
                    num(op, "y0") * k,
                    num(op, "x1") * k,
                    num(op, "y1") * k,
                    num(op, "z") * k,
                );
            }
            "eraseEdge" => {
                let a = prism_ops::vertex_at(&m, v3(&op["p"], k)).expect("first end");
                let b = prism_ops::vertex_at(&m, v3(&op["q"], k)).expect("second end");
                let e = m.edge_between(a, b).expect("edge to erase");
                prism_ops::remove(&mut m, &Selection::edge(e));
            }
            "pushpull" => {
                let f = prism_ops::face_at(&m, v3(&op["faceAt"], k)).expect("face to pull");
                prism_ops::push_pull(
                    &mut m,
                    f,
                    num(op, "d") * k,
                    op["loop"].as_bool().unwrap_or(false),
                );
            }
            other => panic!("unknown op {other}"),
        }
    }
    m
}

/// A face as the golden file describes it: loop size, hole sizes, area.
fn describe(m: &Model) -> Vec<(usize, Vec<usize>, f64)> {
    let mut v: Vec<_> = m
        .faces()
        .map(|(id, f)| {
            let mut holes: Vec<usize> = f.holes.iter().map(|h| h.len()).collect();
            holes.sort();
            (f.outer.len(), holes, m.face_area(id))
        })
        .collect();
    v.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    v
}

fn check(case: &Value, k: f64) -> Result<(), String> {
    let id = case["id"].as_str().unwrap_or("?");
    let m = run(case, k);
    let e = &case["expect"];
    let want = |key: &str| e[key].as_u64().unwrap_or(0) as usize;
    let got = (m.vertex_count(), m.edge_count(), m.face_count());
    let expected = (want("vertices"), want("edges"), want("faces"));
    if got != expected {
        return Err(format!(
            "{id} at scale {k}: vertices, edges, faces {got:?}, expected {expected:?}\n{:?}",
            describe(&m)
        ));
    }
    let mut exp: Vec<(usize, Vec<usize>, f64)> = e["face_list"]
        .as_array()
        .expect("face_list")
        .iter()
        .map(|f| {
            let mut holes: Vec<usize> = f["holes"]
                .as_array()
                .expect("holes")
                .iter()
                .map(|h| h.as_u64().unwrap_or(0) as usize)
                .collect();
            holes.sort();
            (
                f["loop"].as_u64().unwrap_or(0) as usize,
                holes,
                num(f, "area") * k * k,
            )
        })
        .collect();
    exp.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    let ours = describe(&m);
    let area_tol = 1e-6 * k * k * 100.0;
    for (o, x) in ours.iter().zip(&exp) {
        if o.0 != x.0 || o.1 != x.1 || (o.2 - x.2).abs() > area_tol {
            return Err(format!(
                "{id} at scale {k}: faces {ours:?}, expected {exp:?}"
            ));
        }
    }
    let (lo, hi) = m.bbox().ok_or(format!("{id}: empty model"))?;
    let (blo, bhi) = (v3(&e["bbox"]["min"], k), v3(&e["bbox"]["max"], k));
    let tol = m.tol().dist * 10.0;
    if lo.distance(blo) > tol || hi.distance(bhi) > tol {
        return Err(format!(
            "{id} at scale {k}: bbox {lo}..{hi}, expected {blo}..{bhi}"
        ));
    }
    let problems = prism_ops::check::problems(&m);
    if !problems.is_empty() {
        return Err(format!("{id} at scale {k}: rules broken: {problems:?}"));
    }
    Ok(())
}

#[test]
fn verified_golden_cases_pass_at_every_scale() {
    let all = cases();
    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    let mut ran = BTreeSet::new();
    for case in all["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().unwrap_or("?").to_owned();
        if case["status"] != "verified" {
            skipped.push(id);
            continue;
        }
        for k in SCALES {
            if let Err(e) = check(case, k) {
                failures.push(e);
            }
        }
        ran.insert(id);
    }
    println!(
        "ran {} cases at {} scales; skipped (not verified): {skipped:?}",
        ran.len(),
        SCALES.len()
    );
    assert!(ran.len() >= 18, "only {} verified cases ran", ran.len());
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_same_case_gives_the_same_model_every_run() {
    let all = cases();
    for case in all["cases"].as_array().expect("cases") {
        assert_eq!(run(case, 1.0).to_json(), run(case, 1.0).to_json());
    }
}

#[test]
#[ignore = "prints the cases that are not verified, for review"]
fn show_unverified_cases() {
    let all = cases();
    for case in all["cases"].as_array().expect("cases") {
        if case["status"] != "verified" {
            let m = run(case, 1.0);
            println!("{}: {:?}", case["id"], check(case, 1.0));
            println!(
                "  ours: {} vertices, {} edges, {} faces {:?}",
                m.vertex_count(),
                m.edge_count(),
                m.face_count(),
                describe(&m)
            );
        }
    }
}
