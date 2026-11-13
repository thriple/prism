//! The .prism file: exact round trips, reproducible bytes, versions and
//! damaged files (brief 08, "Tests").

use prism_doc::Command;
use prism_file::{CameraData, Contents, FileError, Journal, Settings, read, write};
use prism_math::{DQuat, DVec3, DisplayUnits, LengthUnit};
use prism_topo::{Guide, Model};

fn rich() -> Model {
    let mut d = prism_tools::scenes::pocket();
    // Rotated, so coordinates are not round numbers.
    let all = d.model().vertices().map(|(id, _)| id).collect();
    d.run(Command::Rotate {
        vertices: all,
        centre: DVec3::new(0.3, -0.7, 0.1),
        axis: DVec3::new(1.0, 2.0, 3.0),
        angle: 0.731,
    });
    d.run(Command::AddGuide {
        guide: Guide::Line {
            origin: DVec3::new(0.1, 0.2, 0.3),
            dir: DQuat::from_rotation_z(0.3) * DVec3::X,
        },
    });
    d.run(Command::AddGuide {
        guide: Guide::Point {
            at: DVec3::new(1.0 / 3.0, 2.0, 0.0),
        },
    });
    d.model().clone()
}

fn contents(model: Model) -> Contents {
    Contents {
        assembly: prism_doc::Assembly::with_model(model),
        settings: Settings {
            units: DisplayUnits {
                length: LengthUnit::FeetInches,
                decimals: 2,
            },
            camera: Some(CameraData {
                parallel: true,
                fov: Some(50.0),
                eye: [1.0 / 7.0, -8.0, 9.0],
                target: [0.0, 0.1, 0.2],
            }),
            ..Settings::default()
        },
    }
}

fn models() -> Vec<Model> {
    vec![
        Model::new(),
        prism_tools::scenes::cube().model().clone(),
        prism_tools::scenes::pocket().model().clone(),
        prism_tools::scenes::plate().model().clone(),
        prism_tools::scenes::large(8).model().clone(),
        rich(),
    ]
}

#[test]
fn round_trips_are_exact() {
    for m in models() {
        let c = contents(m);
        let back = read(&write(&c, "test")).expect("opens");
        // Bit for bit: the JSON form writes every f64 exactly.
        assert_eq!(back.model().to_json(), c.model().to_json());
        assert_eq!(back.model().counters(), c.model().counters());
        assert_eq!(back, c);
    }
}

#[test]
fn the_same_document_gives_the_same_bytes() {
    let c = contents(rich());
    assert_eq!(write(&c, "test"), write(&c.clone(), "test"));
}

#[test]
fn json_numbers_come_back_bit_for_bit() {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..20_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let f = f64::from_bits(x);
        if !f.is_finite() {
            continue;
        }
        let back: f64 = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(back.to_bits(), f.to_bits(), "{f:e}");
    }
}

#[test]
fn unknown_document_fields_survive() {
    let mut c = contents(Model::new());
    c.settings
        .extra
        .insert("future".into(), serde_json::json!({"a": [1, 2]}));
    let back = read(&write(&c, "test")).expect("opens");
    assert_eq!(back.settings.extra, c.settings.extra);
}

#[test]
fn a_newer_file_is_refused_and_an_older_one_migrates() {
    let c = contents(rich());
    let bytes = write(&c, "test");
    let mut raw = zip_entries(&bytes);
    let manifest = raw.iter_mut().find(|(n, _)| n == "manifest.json").unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&manifest.1).unwrap();
    v["schema"] = serde_json::json!(prism_file::SCHEMA + 1);
    manifest.1 = serde_json::to_vec(&v).unwrap();
    let newer = rezip(&raw);
    assert_eq!(
        read(&newer),
        Err(FileError::Newer {
            schema: prism_file::SCHEMA + 1
        })
    );
    let old = prism_file::schema_0_for_tests(c.model());
    let back = read(&old).expect("migrates");
    assert_eq!(back.model().to_json(), c.model().to_json());
    assert_eq!(back.settings, Settings::default());
}

#[test]
fn not_prism_files_and_damage_are_refused() {
    assert!(matches!(read(b""), Err(FileError::NotPrism(_))));
    assert!(matches!(read(b"hello"), Err(FileError::NotPrism(_))));
    let bytes = write(&contents(rich()), "test");
    for cut in [10, bytes.len() / 2, bytes.len() - 1] {
        assert!(read(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    // A zip without our manifest.
    let other = rezip(&[("readme.txt".into(), b"hi".to_vec())]);
    assert!(matches!(read(&other), Err(FileError::NotPrism(_))));
    // Geometry whose face uses a missing edge.
    let mut raw = zip_entries(&bytes);
    let geo = raw
        .iter_mut()
        .find(|(n, _)| n == "geometry/model.bin")
        .unwrap();
    geo.1.truncate(geo.1.len() / 2);
    assert!(matches!(read(&rezip(&raw)), Err(FileError::Damaged(_))));
}

#[test]
fn the_recovery_journal_replays() {
    let base = prism_tools::scenes::cube().model().clone();
    let mut d = prism_doc::Document::with_model(base.clone());
    let cmds = [
        Command::Rect {
            x0: 2.0,
            y0: 2.0,
            x1: 3.0,
            y1: 3.5,
            z: 0.0,
        },
        Command::Segment {
            p: DVec3::new(2.0, 2.5, 0.0),
            q: DVec3::new(3.0, 2.7, 0.0),
        },
    ];
    for c in &cmds {
        d.run(c.clone());
    }
    let j = Journal {
        path: Some("C:/models/a.prism".into()),
        commands: d.log().cloned().collect(),
    };
    let back = Journal::from_json(&j.to_json()).expect("reads");
    assert_eq!(back, j);
    assert_eq!(
        back.restore(prism_doc::Assembly::with_model(base))
            .model()
            .to_json(),
        d.model().to_json()
    );
}

fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut d = Vec::new();
            f.read_to_end(&mut d).unwrap();
            (f.name().unwrap().into_owned(), d)
        })
        .collect()
}

fn rezip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, d) in files {
        z.start_file(n.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(d).unwrap();
    }
    z.finish().unwrap().into_inner()
}

#[cfg(not(target_arch = "wasm32"))]
mod fuzz {
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]

        /// Flipping bytes anywhere never panics: the file opens or is refused.
        #[test]
        fn damaged_bytes_never_panic(flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8)) {
            let mut bytes = prism_file::write(&super::contents(super::rich()), "test");
            for (i, x) in flips {
                let k = i.index(bytes.len());
                bytes[k] ^= x | 1;
            }
            let _ = prism_file::read(&bytes);
        }
    }
}

#[test]
fn names_tags_hidden_and_locked_flags_round_trip() {
    use prism_doc::{Command, Document};
    let mut d = Document::new();
    d.run(Command::Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
        z: 0.0,
    });
    let m = d.model();
    let sel = prism_ops::Selection {
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
    };
    let g = d.assembly().next_instance();
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    d.run(Command::NameInstance {
        instance: g,
        name: "Plate".into(),
    });
    d.run(Command::AddTag {
        name: "Parts".into(),
    });
    d.run(Command::SetTag {
        instances: [g].into(),
        tag: Some(prism_doc::TagId(0)),
    });
    d.run(Command::SetTagVisible {
        tag: prism_doc::TagId(0),
        visible: false,
    });
    d.run(Command::SetHidden {
        instances: [g].into(),
        hidden: true,
    });
    // Locks too (schema 11).
    d.run(Command::SetLocked {
        instances: [g].into(),
        locked: true,
    });
    assert!(d.assembly().is_locked(g));
    let c = Contents {
        assembly: d.assembly().clone(),
        settings: Default::default(),
    };
    let bytes = prism_file::write(&c, "test");
    let back = prism_file::read(&bytes).expect("reads");
    assert_eq!(back.assembly, c.assembly);
    assert_eq!(prism_file::write(&back, "test"), bytes);
}

/// Curved faces are saved and read back exactly (Exact Curves 4a).
#[test]
fn cylinders_round_trip_exactly() {
    let m = prism_tools::scenes::cylinder(1.25, 0.5);
    let c = Contents::with_model(m.clone(), Settings::default());
    let back = read(&write(&c, "test")).expect("opens");
    assert_eq!(back.model().to_json(), m.to_json());
    assert_eq!(
        back.model().faces().filter(|(_, f)| f.is_curved()).count(),
        2
    );
}

/// Soft and smooth edges are kept (Exact Curves 4c).
#[test]
fn soft_edges_round_trip() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a disc");
    prism_ops::push_pull(&mut m, f, 1.0, false);
    assert_eq!(m.edges().filter(|(_, e)| e.soft && e.smooth).count(), 2);
    let c = Contents::with_model(m.clone(), Settings::default());
    let back = read(&write(&c, "test")).expect("opens");
    assert_eq!(back.model().to_json(), m.to_json());
}
