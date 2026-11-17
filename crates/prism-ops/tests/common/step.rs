//! STEP round trips for the tests (Solids & CAD part 5a, brief 44).
#![allow(dead_code)]

use std::collections::BTreeSet;

use prism_kernel::step::{self, Node, Shape};
use prism_math::{DAffine3, LengthUnit};
use prism_ops::bridge::{put_back, region};
use prism_ops::validate::validate;
use prism_topo::Model;

/// Every valid solid of `m` as one part.
pub fn shapes(m: &Model) -> Vec<Shape> {
    validate(m)
        .iter()
        .filter(|c| c.valid)
        .map(|c| Shape {
            region: region(m, &c.faces).expect("a region"),
            open: false,
            colours: Vec::new(),
        })
        .collect()
}

/// A node named `name` holding `shapes`, at `place`.
pub fn node(name: &str, place: DAffine3, shapes: Vec<usize>) -> Node {
    Node {
        name: name.into(),
        product: None,
        place,
        colour: None,
        shapes,
        children: Vec::new(),
    }
}

/// The shapes read from `text`, put into a new model.
pub fn read_into_model(text: &str, tol: f64) -> Model {
    let back = step::read(text, tol).expect("a STEP file");
    let mut out = Model::new();
    for s in back.shapes {
        let s = s.expect("read");
        put_back(&mut out, &BTreeSet::new(), &s.region).expect("stored");
    }
    out
}

/// `m`'s solids through a STEP file in `unit` and back: the new model,
/// and the file.
pub fn through_step(m: &Model, unit: LengthUnit) -> (Model, String) {
    let s = shapes(m);
    let root = node("part", DAffine3::IDENTITY, (0..s.len()).collect());
    let w = step::write(&s, &[root], unit, m.tol().dist).expect("written");
    assert_eq!(w.left_out, Vec::new());
    (read_into_model(&w.text, m.tol().dist), w.text)
}

/// The kinds of the faces' surfaces, sorted.
pub fn kinds(m: &Model) -> Vec<String> {
    let mut k: Vec<String> = m
        .faces()
        .map(|(_, f)| {
            let d = format!("{:?}", f.surface);
            d.chars().take_while(|c| c.is_alphanumeric()).collect()
        })
        .collect();
    k.sort();
    k
}

/// The volumes of `m`'s solids, sorted, all of them valid.
pub fn volumes(m: &Model) -> Vec<f64> {
    let all = validate(m);
    assert!(all.iter().all(|c| c.valid), "{all:?}");
    let mut v: Vec<f64> = all.iter().map(|c| c.volume.unwrap()).collect();
    v.sort_by(f64::total_cmp);
    v
}

/// `m`'s solids come back from STEP in `unit` with the same faces and
/// volumes.
pub fn same_through_step(m: &Model, unit: LengthUnit) -> Model {
    let (back, _) = through_step(m, unit);
    assert_eq!(kinds(&back), kinds(m));
    let (a, b) = (volumes(m), volumes(&back));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() <= 1e-9 * x.abs().max(1.0), "{a:?} vs {b:?}");
    }
    back
}
