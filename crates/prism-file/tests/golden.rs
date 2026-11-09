//! Golden files: `.prism` files written by earlier versions must still open,
//! and saving them again gives the same bytes. Regenerate (only when the
//! format changes on purpose, with a new schema) with PRISM_WRITE_GOLDEN=1.

use prism_file::{Contents, Settings, read, write};
use prism_math::{DisplayUnits, LengthUnit};

const APP: &str = "golden";

fn dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/files")
}

fn cases() -> Vec<(&'static str, Contents)> {
    let mut pocket = prism_tools::scenes::pocket();
    pocket.run(prism_doc::Command::AddGuide {
        guide: prism_topo::Guide::Line {
            origin: prism_math::DVec3::new(0.0, -1.5, 0.0),
            dir: prism_math::DVec3::X,
        },
    });
    vec![
        (
            "cube.prism",
            Contents::with_model(
                prism_tools::scenes::cube().model().clone(),
                Settings::default(),
            ),
        ),
        (
            "pocket-with-guide.prism",
            Contents::with_model(
                pocket.model().clone(),
                Settings {
                    units: DisplayUnits {
                        length: LengthUnit::Millimetres,
                        decimals: 0,
                    },
                    ..Settings::default()
                },
            ),
        ),
        (
            "plate.prism",
            Contents::with_model(
                prism_tools::scenes::plate().model().clone(),
                Settings::default(),
            ),
        ),
        (
            "groups.prism",
            Contents {
                assembly: groups().assembly().clone(),
                settings: Settings::default(),
            },
        ),
        (
            "components.prism",
            Contents {
                assembly: components().assembly().clone(),
                settings: Settings::default(),
            },
        ),
        (
            "circle-in-a-plate.prism",
            Contents::with_model(circle_in_a_plate(), Settings::default()),
        ),
    ]
}

/// A square plate with an exact circle in it: a hole and a disc (Exact
/// Curves part 2).
fn circle_in_a_plate() -> prism_topo::Model {
    let mut m = prism_topo::Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    prism_ops::add_circle(
        &mut m,
        prism_math::DVec3::new(2.0, 2.0, 0.0),
        prism_math::DVec3::Z,
        1.0,
    );
    m
}

/// A box made into a component "Box", with two linked copies, one made
/// unique, and one mirrored.
fn components() -> prism_doc::Document {
    use prism_doc::{Change, Command};
    let mut d = prism_tools::scenes::cube();
    let a = d.assembly().next_instance();
    let m = d.model();
    let sel = prism_ops::Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: Default::default(),
    };
    d.run(Command::MakeComponent {
        selection: sel,
        instances: Default::default(),
        name: "Box".into(),
    });
    let b = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [a].into(),
        step: Change::Translate {
            by: prism_math::DVec3::new(5.0, 0.0, 0.0),
        },
        count: 2,
    });
    d.run(Command::MakeUnique {
        instances: [b].into(),
    });
    d.run(Command::Place {
        instances: [a].into(),
        change: Change::Scale {
            centre: prism_math::DVec3::ZERO,
            factor: prism_math::DVec3::new(-1.0, 1.0, 1.0),
        },
    });
    d
}

/// A box made into a group, copied twice, with a group nested in a copy
/// and a line drawn inside one.
fn groups() -> prism_doc::Document {
    use prism_doc::{Change, Command};
    let mut d = prism_tools::scenes::cube();
    let g = d.assembly().next_instance();
    let m = d.model();
    let sel = prism_ops::Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: Default::default(),
    };
    d.run(Command::MakeGroup {
        selection: sel,
        instances: Default::default(),
    });
    let copy = d.assembly().next_instance();
    d.run(Command::CopyInstances {
        instances: [g].into(),
        step: Change::Rotate {
            centre: prism_math::DVec3::new(3.0, 0.0, 0.0),
            axis: prism_math::DVec3::Z,
            angle: 0.5,
        },
        count: 2,
    });
    d.run(Command::MakeGroup {
        selection: Default::default(),
        instances: [g, copy].into(),
    });
    d
}

#[test]
fn golden_files_open_and_save_the_same() {
    if std::env::var_os("PRISM_WRITE_GOLDEN").is_some() {
        for (name, c) in cases() {
            std::fs::write(dir().join(name), write(&c, APP)).expect("write");
        }
    }
    let mut seen = 0;
    for e in std::fs::read_dir(dir()).expect("docs/acceptance/files") {
        let path = e.expect("entry").path();
        if path.extension().is_none_or(|x| x != "prism") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read");
        let c = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let problems = prism_ops::check::problems(c.model());
        assert!(problems.is_empty(), "{}: {problems:?}", path.display());
        assert_eq!(write(&c, APP), bytes, "{} saves the same", path.display());
        seen += 1;
    }
    assert!(seen >= 3, "the golden files are there");
}

/// Files saved before loops named their edges (schema 2, geometry layout 1)
/// open with the same faces and edges, found from the loops. Files saved
/// before arcs (schema 3, layout 2) open the same too.
#[test]
fn schema_2_and_3_files_open_as_now() {
    let mut seen = 0;
    let older = ["schema-2", "schema-3"].map(|d| std::fs::read_dir(dir().join(d)).expect(d));
    for e in older.into_iter().flatten() {
        let path = e.expect("entry").path();
        let old = read(&std::fs::read(&path).expect("read"))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = path.file_name().expect("name");
        let new = read(&std::fs::read(dir().join(name)).expect("read")).expect("reads");
        assert_eq!(old.model(), new.model(), "{}", path.display());
        assert!(prism_ops::check::problems(old.model()).is_empty());
        assert_eq!(write(&old, APP), write(&new, APP), "{}", path.display());
        seen += 1;
    }
    assert!(seen >= 10);
}

/// Files saved before groups (schema 1) still open, as they were.
#[test]
fn schema_1_files_still_open() {
    let mut seen = 0;
    for e in std::fs::read_dir(dir().join("schema-1")).expect("docs/acceptance/files/schema-1") {
        let path = e.expect("entry").path();
        let bytes = std::fs::read(&path).expect("read");
        let c = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(c.model().face_count() > 0, "{}", path.display());
        assert!(!c.assembly.has_instances());
        seen += 1;
    }
    assert!(seen >= 3);
}
