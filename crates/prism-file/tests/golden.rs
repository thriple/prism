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
            Contents {
                model: prism_tools::scenes::cube().model().clone(),
                settings: Settings::default(),
            },
        ),
        (
            "pocket-with-guide.prism",
            Contents {
                model: pocket.model().clone(),
                settings: Settings {
                    units: DisplayUnits {
                        length: LengthUnit::Millimetres,
                        decimals: 0,
                    },
                    ..Settings::default()
                },
            },
        ),
        (
            "plate.prism",
            Contents {
                model: prism_tools::scenes::plate().model().clone(),
                settings: Settings::default(),
            },
        ),
    ]
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
        let problems = prism_ops::check::problems(&c.model);
        assert!(problems.is_empty(), "{}: {problems:?}", path.display());
        assert_eq!(write(&c, APP), bytes, "{} saves the same", path.display());
        seen += 1;
    }
    assert!(seen >= 3, "the golden files are there");
}
