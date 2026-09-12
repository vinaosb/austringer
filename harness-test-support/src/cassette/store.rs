//! Cassette persistence: deterministic JSON save, clear-error load.

use std::path::Path;

use super::error::CassetteError;
use super::record::Cassette;

impl Cassette {
    /// Serializes the cassette to `path` as pretty JSON with sorted map keys.
    pub fn save(&self, path: &Path) -> Result<(), CassetteError> {
        let json =
            serde_json::to_string_pretty(self).map_err(|source| CassetteError::Serialize {
                path: path.to_owned(),
                source,
            })?;
        std::fs::write(path, json).map_err(|source| CassetteError::Io {
            path: path.to_owned(),
            source,
        })
    }

    /// Loads a cassette, aborting with [`CassetteError::Corrupt`] (message
    /// names the file and the JSON error with line/column) on any damage.
    pub fn load(path: &Path) -> Result<Self, CassetteError> {
        let raw = std::fs::read_to_string(path).map_err(|source| CassetteError::Io {
            path: path.to_owned(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| CassetteError::Corrupt {
            path: path.to_owned(),
            source,
        })
    }
}
