//! The `.prism` file (PLAN 1.13): a document to bytes and back, exactly.
//!
//! A `.prism` file is a zip holding:
//! - `manifest.json`: the format, the schema version, the app that wrote it,
//!   the units and the list of entries;
//! - `document.json`: settings that are not geometry (units, camera). Fields
//!   this version does not know are kept and written back;
//! - `geometry/model.bin`: the model in postcard, with its stable ids and id
//!   counters. Its layout is its own versioned struct here, not the
//!   in-memory `Model`, so the two can change separately.
//!
//! Pure: no clock, no files, no randomness. The same document gives the
//! same bytes, because every zip entry has a fixed timestamp.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use prism_math::DisplayUnits;
use prism_topo::Model;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

mod geometry;
pub mod mesh;
mod migrate;

#[doc(hidden)]
pub use migrate::schema_0_for_tests;
pub use migrate::{Raw, migrate};

/// The schema this version writes. Older schemas are migrated on open;
/// newer ones are refused.
pub const SCHEMA: u32 = 1;
/// The `format` field of every manifest.
pub const FORMAT: &str = "prism";
/// The file extension.
pub const EXTENSION: &str = "prism";

const MANIFEST: &str = "manifest.json";
const DOCUMENT: &str = "document.json";
const MODEL: &str = "geometry/model.bin";
/// The largest entry read (a damaged size field must not exhaust memory).
const MAX_ENTRY: u64 = 1 << 32;

/// Where the camera was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraData {
    pub eye: [f64; 3],
    pub target: [f64; 3],
}

/// What a file holds besides the geometry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub units: DisplayUnits,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraData>,
    /// Fields written by a newer version of Prism, kept as they were.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A document as a file holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contents {
    pub model: Model,
    pub settings: Settings,
}

/// Why a file could not be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileError {
    /// Not a `.prism` file at all.
    NotPrism(String),
    /// Written by a newer Prism.
    Newer { schema: u32 },
    /// A `.prism` file, but damaged.
    Damaged(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::NotPrism(why) => write!(f, "this is not a Prism file ({why})"),
            FileError::Newer { schema } => write!(
                f,
                "this file was made by a newer version of Prism (file format {schema}; this version reads up to {SCHEMA}). Update Prism to open it"
            ),
            FileError::Damaged(why) => write!(f, "the file is damaged ({why})"),
        }
    }
}

impl std::error::Error for FileError {}

/// One entry of the manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub kind: String,
    pub version: u32,
}

/// The manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub schema: u32,
    pub app: String,
    pub units: String,
    pub entries: Vec<Entry>,
}

/// The `.prism` bytes for a document. `app` is the version of Prism
/// writing it.
pub fn write(c: &Contents, app: &str) -> Vec<u8> {
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        schema: SCHEMA,
        app: app.to_owned(),
        units: c.settings.units.length.name().to_owned(),
        entries: vec![
            Entry {
                path: DOCUMENT.to_owned(),
                kind: "document".to_owned(),
                version: 1,
            },
            Entry {
                path: MODEL.to_owned(),
                kind: "geometry".to_owned(),
                version: geometry::VERSION,
            },
        ],
    };
    let mut files: Vec<(&str, Vec<u8>)> = vec![
        (MANIFEST, pretty(&manifest)),
        (DOCUMENT, pretty(&c.settings)),
        (MODEL, geometry::encode(&c.model)),
    ];
    // A fixed order, so the same document gives the same bytes.
    files[1..].sort_by(|a, b| a.0.cmp(b.0));
    zip_files(&files)
}

/// Open `.prism` bytes.
pub fn read(bytes: &[u8]) -> Result<Contents, FileError> {
    let raw = unzip(bytes)?;
    let raw = migrate(raw)?;
    let doc = raw
        .files
        .get(DOCUMENT)
        .ok_or_else(|| FileError::Damaged(format!("no {DOCUMENT}")))?;
    let settings: Settings =
        serde_json::from_slice(doc).map_err(|e| FileError::Damaged(format!("{DOCUMENT}: {e}")))?;
    let version = raw
        .entry(MODEL)
        .map(|e| e.version)
        .ok_or_else(|| FileError::Damaged(format!("no {MODEL} in the manifest")))?;
    let bin = raw
        .files
        .get(MODEL)
        .ok_or_else(|| FileError::Damaged(format!("no {MODEL}")))?;
    let model = geometry::decode(bin, version)?;
    Ok(Contents { model, settings })
}

fn pretty<T: Serialize>(v: &T) -> Vec<u8> {
    let mut out = serde_json::to_vec_pretty(v).unwrap_or_default();
    out.push(b'\n');
    out
}

/// A zip of these files, deflated, with fixed timestamps and permissions.
pub(crate) fn zip_files(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        // The same "made by" system everywhere (zip marks Windows files as
        // DOS otherwise), so the bytes do not depend on the platform.
        .system(zip::System::Unix)
        .unix_permissions(0o644);
    for (name, data) in files {
        // Writing to memory cannot fail.
        let _ = z.start_file(*name, opts);
        let _ = z.write_all(data);
    }
    z.finish().map(Cursor::into_inner).unwrap_or_default()
}

/// The files in a zip, and its manifest.
fn unzip(bytes: &[u8]) -> Result<Raw, FileError> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| FileError::NotPrism(format!("not a zip: {e}")))?;
    let mut files = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z
            .by_index(i)
            .map_err(|e| FileError::Damaged(format!("entry {i}: {e}")))?;
        if f.is_dir() {
            continue;
        }
        let name = f
            .name()
            .map_err(|e| FileError::Damaged(format!("entry {i}'s name: {e}")))?
            .into_owned();
        let mut data = Vec::new();
        (&mut f)
            .take(MAX_ENTRY)
            .read_to_end(&mut data)
            .map_err(|e| FileError::Damaged(format!("{name}: {e}")))?;
        files.insert(name, data);
    }
    let m = files
        .get(MANIFEST)
        .ok_or_else(|| FileError::NotPrism(format!("no {MANIFEST}")))?;
    let manifest: Value =
        serde_json::from_slice(m).map_err(|e| FileError::Damaged(format!("{MANIFEST}: {e}")))?;
    if manifest.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(FileError::NotPrism("the manifest is not Prism's".into()));
    }
    Ok(Raw { manifest, files })
}

/// The recovery journal: the commands run since the base model (the last
/// save or open), as JSON. The base model is kept beside it as a `.prism`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Journal {
    /// Where the document was saved, if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The commands run since the base model, oldest first.
    #[serde(default)]
    pub commands: Vec<prism_doc::Command>,
}

impl Journal {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Journal, FileError> {
        serde_json::from_str(text).map_err(|e| FileError::Damaged(format!("journal: {e}")))
    }

    /// The document the journal brings back: the base model with the
    /// commands run on it again (an empty undo history before them).
    pub fn restore(&self, base: Model) -> prism_doc::Document {
        let mut d = prism_doc::Document::with_model(base);
        for c in &self.commands {
            d.run(c.clone());
        }
        d
    }
}
