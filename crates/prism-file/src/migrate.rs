//! Opening files from older schemas: one migration per version step.
//!
//! Schema 0 is made up (no file was ever written with it). It stands for
//! the first JSON-only layout, `model.json`, so the machinery is tested
//! before a real second schema exists.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{DOCUMENT, Entry, FileError, MODEL, SCHEMA, geometry};

/// A file's manifest and entries, before they are read into a document.
#[derive(Clone, Debug, PartialEq)]
pub struct Raw {
    pub manifest: Value,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Raw {
    pub fn schema(&self) -> Option<u32> {
        self.manifest
            .get("schema")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
    }

    pub(crate) fn entry(&self, path: &str) -> Option<Entry> {
        let list = self.manifest.get("entries")?.as_array()?;
        list.iter()
            .filter_map(|e| serde_json::from_value::<Entry>(e.clone()).ok())
            .find(|e| e.path == path)
    }
}

/// Bring a file up to [`SCHEMA`], one step at a time.
pub fn migrate(mut raw: Raw) -> Result<Raw, FileError> {
    let mut schema = raw
        .schema()
        .ok_or_else(|| FileError::Damaged("the manifest has no schema".into()))?;
    if schema > SCHEMA {
        return Err(FileError::Newer { schema });
    }
    while schema < SCHEMA {
        raw = match schema {
            0 => from_0(raw)?,
            // Schema 2 adds groups; a schema 1 file has none.
            1 => raw,
            // Schema 3 writes geometry layout 2; layout 1 entries are read
            // by finding each loop's edges (`geometry::decode`).
            2 => raw,
            // Schema 4 writes geometry layout 3 (arcs); older layouts are
            // read as they are.
            3 => raw,
            // Schema 5 writes geometry layout 4 (soft and smooth edges).
            4 => raw,
            // Schema 6 writes geometry layout 5 (cones, spheres and tori),
            // which reads layout 4 as it is.
            5 => raw,
            // Schema 7 writes geometry layout 6 (approximate edges), which
            // reads layout 5 with none.
            6 => raw,
            s => return Err(FileError::Damaged(format!("no migration from schema {s}"))),
        };
        schema += 1;
        raw.manifest["schema"] = json!(schema);
    }
    Ok(raw)
}

/// Schema 0: the model as JSON in `model.json`, and no `document.json`.
fn from_0(mut raw: Raw) -> Result<Raw, FileError> {
    let json = raw
        .files
        .remove("model.json")
        .ok_or_else(|| FileError::Damaged("no model.json".into()))?;
    let text = std::str::from_utf8(&json).map_err(|e| FileError::Damaged(e.to_string()))?;
    let model = prism_topo::Model::from_json(text).map_err(FileError::Damaged)?;
    raw.files.insert(MODEL.to_owned(), geometry::encode(&model));
    raw.files
        .entry(DOCUMENT.to_owned())
        .or_insert_with(|| b"{}".to_vec());
    raw.manifest["entries"] = json!([
        {"path": DOCUMENT, "kind": "document", "version": 1},
        {"path": MODEL, "kind": "geometry", "version": geometry::VERSION},
    ]);
    Ok(raw)
}

/// A schema 0 file, for tests of the migration.
pub fn schema_0_for_tests(model: &prism_topo::Model) -> Vec<u8> {
    let manifest = json!({"format": "prism", "schema": 0, "app": "0.0.0", "units": "Metres", "entries": [
        {"path": "model.json", "kind": "geometry", "version": 0}
    ]});
    crate::zip_files(&[
        (
            "manifest.json",
            serde_json::to_vec(&manifest).unwrap_or_default(),
        ),
        ("model.json", model.to_json().into_bytes()),
    ])
}
