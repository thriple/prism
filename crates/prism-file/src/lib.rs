//! The `.prism` file (PLAN 1.13): a document to bytes and back, exactly.
//!
//! A `.prism` file is a zip holding:
//! - `manifest.json`: the format, the schema version, the app that wrote it,
//!   the units and the list of entries;
//! - `document.json`: settings that are not geometry (units, camera). Fields
//!   this version does not know are kept and written back;
//! - `geometry/model.bin`: the root model (outside every group) in postcard,
//!   with its stable ids and id counters. Its layout is its own versioned
//!   struct here, not the in-memory `Model`, so the two can change
//!   separately;
//! - only when there are groups: `groups.json`, the definitions and the
//!   instances that place them, and `geometry/defs/<id>.bin`, each group's
//!   own model in the same layout (schema 2).
//!
//! Pure: no clock, no files, no randomness. The same document gives the
//! same bytes, because every zip entry has a fixed timestamp.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use prism_doc::{Assembly, DefId, Definition, Instance, InstanceId, ROOT};
use prism_math::DAffine3;
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
pub const SCHEMA: u32 = 2;
/// The `format` field of every manifest.
pub const FORMAT: &str = "prism";
/// The file extension.
pub const EXTENSION: &str = "prism";

const MANIFEST: &str = "manifest.json";
const DOCUMENT: &str = "document.json";
const MODEL: &str = "geometry/model.bin";
const GROUPS: &str = "groups.json";

fn def_path(id: u32) -> String {
    format!("geometry/defs/{id}.bin")
}
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
    /// The geometry: the root model and its groups.
    pub assembly: Assembly,
    pub settings: Settings,
}

impl Contents {
    /// A document with no groups.
    pub fn with_model(model: Model, settings: Settings) -> Contents {
        Contents {
            assembly: Assembly::with_model(model),
            settings,
        }
    }

    /// The root model: the geometry outside every group.
    pub fn model(&self) -> &Model {
        &self.assembly.root().model
    }
}

/// `groups.json`: the instance tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct GroupsV1 {
    next_def: u32,
    next_instance: u32,
    /// Every definition, the root (0) included, with what it places.
    defs: Vec<DefV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct DefV1 {
    id: u32,
    #[serde(default)]
    instances: Vec<InstanceV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct InstanceV1 {
    id: u32,
    def: u32,
    /// The placement: the three columns of the matrix, then the move.
    transform: [f64; 12],
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
    let mut manifest = manifest;
    let mut files: Vec<(String, Vec<u8>)> = vec![
        (DOCUMENT.to_owned(), pretty(&c.settings)),
        (MODEL.to_owned(), geometry::encode(c.model())),
    ];
    let asm = &c.assembly;
    if asm.defs().count() > 1 || asm.has_instances() {
        let (next_def, next_instance) = asm.counters();
        let groups = GroupsV1 {
            next_def,
            next_instance,
            defs: asm
                .defs()
                .map(|(id, d)| DefV1 {
                    id: id.0,
                    instances: d
                        .instances
                        .iter()
                        .map(|(i, inst)| InstanceV1 {
                            id: i.0,
                            def: inst.def.0,
                            transform: inst.transform.to_cols_array(),
                        })
                        .collect(),
                })
                .collect(),
        };
        files.push((GROUPS.to_owned(), pretty(&groups)));
        manifest.entries.push(Entry {
            path: GROUPS.to_owned(),
            kind: "groups".to_owned(),
            version: 1,
        });
        for (id, d) in asm.defs().filter(|(id, _)| *id != ROOT) {
            files.push((def_path(id.0), geometry::encode(&d.model)));
            manifest.entries.push(Entry {
                path: def_path(id.0),
                kind: "geometry".to_owned(),
                version: geometry::VERSION,
            });
        }
    }
    // A fixed order, so the same document gives the same bytes.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut all: Vec<(&str, Vec<u8>)> = vec![(MANIFEST, pretty(&manifest))];
    all.extend(files.iter().map(|(p, b)| (p.as_str(), b.clone())));
    zip_files(&all)
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
    let assembly = match raw.files.get(GROUPS) {
        None => Assembly::with_model(model),
        Some(json) => read_groups(&raw, json, model)?,
    };
    Ok(Contents { assembly, settings })
}

/// The groups: `groups.json` and each definition's model.
fn read_groups(raw: &Raw, json: &[u8], root: Model) -> Result<Assembly, FileError> {
    let g: GroupsV1 =
        serde_json::from_slice(json).map_err(|e| FileError::Damaged(format!("{GROUPS}: {e}")))?;
    let mut defs = BTreeMap::new();
    let mut root = Some(root);
    for d in g.defs {
        let model = if d.id == ROOT.0 {
            root.take()
                .ok_or_else(|| FileError::Damaged("the root is listed twice".into()))?
        } else {
            let path = def_path(d.id);
            let version = raw
                .entry(&path)
                .map(|e| e.version)
                .ok_or_else(|| FileError::Damaged(format!("no {path} in the manifest")))?;
            let bin = raw
                .files
                .get(&path)
                .ok_or_else(|| FileError::Damaged(format!("no {path}")))?;
            geometry::decode(bin, version)?
        };
        let instances = d
            .instances
            .iter()
            .map(|i| {
                (
                    InstanceId(i.id),
                    Instance {
                        def: DefId(i.def),
                        transform: DAffine3::from_cols_array(&i.transform),
                    },
                )
            })
            .collect();
        if defs
            .insert(DefId(d.id), Definition { model, instances })
            .is_some()
        {
            return Err(FileError::Damaged(format!(
                "definition {} appears twice",
                d.id
            )));
        }
    }
    Assembly::from_parts(defs, g.next_def, g.next_instance).map_err(FileError::Damaged)
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

    /// The document the journal brings back: the base geometry with the
    /// commands run on it again (an empty undo history before them).
    pub fn restore(&self, base: Assembly) -> prism_doc::Document {
        let mut d = prism_doc::Document::with_assembly(base);
        for c in &self.commands {
            d.run(c.clone());
        }
        d
    }
}
