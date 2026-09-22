//! Derived-projection store: cached projections of stored media under
//! `<session>/media/derived/`, never inside the event JSONL.
//!
//! Each cache entry is a pair: a content-addressed artifact file (the
//! projection bytes) plus a JSON provenance manifest naming the source
//! hash, producer, kind, creation time, and artifact reference. The
//! pair is written atomically with the manifest LAST, so a crash
//! between the two writes leaves an ignorable orphan artifact (the
//! lookup misses) and never a manifest referencing absent bytes.
//! Artifacts are deliberately never garbage-collected; retention is
//! later work.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::media::{write_atomic, ContentHash, MediaRef, MediaStore};

/// Maximum byte length of a producer id (it becomes a filename part).
pub const MAX_PRODUCER_BYTES: usize = 128;

/// The kind of a derived projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    /// Deterministic text rendering of a media file's structure.
    TextProjection,
    /// Speech-to-text transcript of audio.
    Transcript,
    /// Reduced raster rendering of an image or video frame.
    Thumbnail,
    /// Natural-language description of visual content.
    SceneDescription,
}

impl ProjectionKind {
    /// Filesystem slug of this kind (`text_projection`, ...).
    #[must_use]
    pub const fn as_slug(self) -> &'static str {
        match self {
            Self::TextProjection => "text_projection",
            Self::Transcript => "transcript",
            Self::Thumbnail => "thumbnail",
            Self::SceneDescription => "scene_description",
        }
    }
}

impl fmt::Display for ProjectionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_slug())
    }
}

/// Ways a derived cache entry can fail to load.
///
/// A plain miss (no manifest at all) is `Ok(None)`; every structural
/// problem with an entry that does exist is a typed variant — never a
/// silent miss.
#[derive(Debug, thiserror::Error)]
pub enum DerivedCorrupt {
    /// The manifest is not valid JSON (truncated or mangled).
    #[error("derived manifest for {producer}/{kind} is not valid JSON: {reason}")]
    Malformed {
        /// Producer id of the lookup key.
        producer: String,
        /// Projection kind of the lookup key.
        kind: ProjectionKind,
        /// Parser error.
        reason: String,
    },
    /// Manifest fields disagree with the lookup key (source, producer,
    /// or kind was tampered).
    #[error("derived manifest does not match the lookup key {requested}/{producer}/{kind}")]
    KeyMismatch {
        /// Requested source hash.
        requested: ContentHash,
        /// Requested producer id.
        producer: String,
        /// Requested projection kind.
        kind: ProjectionKind,
    },
    /// Artifact bytes hash to something other than the manifest's claim.
    #[error("derived artifact hashes to {actual} but the manifest declares {stored}")]
    ArtifactHash {
        /// Hash declared by the manifest.
        stored: ContentHash,
        /// Hash of the bytes actually found.
        actual: ContentHash,
    },
    /// The manifest references an artifact that is not on disk.
    #[error("derived artifact {stored} referenced by the manifest is missing")]
    MissingArtifact {
        /// Hash declared by the manifest.
        stored: ContentHash,
    },
    /// Underlying i/o failure.
    #[error("derived store i/o failure: {0}")]
    Io(#[from] io::Error),
}

/// On-disk provenance record for one derived entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DerivedManifest {
    /// Hash of the source media the projection was derived from.
    source: ContentHash,
    /// Producer id (tool, projector, or model) that made it.
    producer: String,
    /// Projection kind.
    kind: ProjectionKind,
    /// Creation time, milliseconds since the Unix epoch.
    created_ms: u64,
    /// Reference to the artifact bytes.
    artifact: MediaRef,
}

/// Content-addressed cache of derived projections for one session.
pub struct DerivedStore {
    artifacts_dir: PathBuf,
    manifests_dir: PathBuf,
}

impl DerivedStore {
    /// Directory name of the derived store inside the session media dir.
    pub const DIR_NAME: &'static str = "derived";

    /// Open (creating) `<session>/media/derived/` and its subdirectories.
    pub fn open(session_dir: &Path) -> io::Result<Self> {
        let root = session_dir.join(MediaStore::DIR_NAME).join(Self::DIR_NAME);
        let store = Self {
            artifacts_dir: root.join("artifacts"),
            manifests_dir: root.join("manifests"),
        };
        fs::create_dir_all(&store.artifacts_dir)?;
        fs::create_dir_all(&store.manifests_dir)?;
        Ok(store)
    }

    /// Store a derived projection with provenance, returning the
    /// artifact reference.
    ///
    /// Artifacts are content-addressed: identical bytes dedup across
    /// every entry, and re-storing the same key with the same bytes
    /// writes nothing at all (`created_ms` stays stable). Re-storing
    /// with new bytes points the manifest at the new artifact and
    /// deliberately leaves the old artifact in place (no garbage
    /// collection, by design). The manifest is written LAST: a crash
    /// between the two writes leaves an ignorable orphan artifact,
    /// never a dangling reference.
    pub fn store_derived(
        &self,
        source: &ContentHash,
        producer: &str,
        kind: ProjectionKind,
        bytes: &[u8],
    ) -> io::Result<MediaRef> {
        if producer.is_empty() || producer.len() > MAX_PRODUCER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("producer id must be 1..={MAX_PRODUCER_BYTES} bytes, got {producer:?}"),
            ));
        }
        let artifact = MediaRef {
            hash: ContentHash::from_bytes(bytes),
            bytes: bytes.len() as u64,
        };
        // Dedup fast path: an already-valid entry holding exactly these
        // bytes writes nothing. A corrupt manifest is not an error here;
        // the fresh write below overwrites (repairs) it.
        if let Ok(Some(existing)) = self.load_derived(source, producer, kind)
            && ContentHash::from_bytes(&existing) == artifact.hash
        {
            return Ok(artifact);
        }
        let artifact_path = self.artifacts_dir.join(artifact.hash.hex());
        if !artifact_path.exists() {
            write_atomic(&self.artifacts_dir, &artifact_path, bytes)?;
        }
        let manifest = DerivedManifest {
            source: *source,
            producer: producer.to_owned(),
            kind,
            created_ms: unix_epoch_ms()?,
            artifact,
        };
        let encoded = serde_json::to_vec(&manifest)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_atomic(
            &self.manifests_dir,
            &self.manifest_path(source, producer, kind),
            &encoded,
        )?;
        Ok(artifact)
    }

    /// Cache lookup by (source, producer, kind): the projection bytes
    /// when cached, `Ok(None)` on a plain miss. A manifest that exists
    /// but is truncated, key-tampered, or points at missing or modified
    /// bytes is a typed [`DerivedCorrupt`] — never a silent miss.
    pub fn load_derived(
        &self,
        source: &ContentHash,
        producer: &str,
        kind: ProjectionKind,
    ) -> Result<Option<Vec<u8>>, DerivedCorrupt> {
        let Some(raw) = read_optional(&self.manifest_path(source, producer, kind))? else {
            return Ok(None);
        };
        let manifest: DerivedManifest = serde_json::from_slice(&raw).map_err(|error| {
            DerivedCorrupt::Malformed {
                producer: producer.to_owned(),
                kind,
                reason: error.to_string(),
            }
        })?;
        if manifest.source != *source || manifest.producer != producer || manifest.kind != kind {
            return Err(DerivedCorrupt::KeyMismatch {
                requested: *source,
                producer: producer.to_owned(),
                kind,
            });
        }
        let Some(bytes) = read_optional(&self.artifacts_dir.join(manifest.artifact.hash.hex()))?
        else {
            return Err(DerivedCorrupt::MissingArtifact {
                stored: manifest.artifact.hash,
            });
        };
        let actual = ContentHash::from_bytes(&bytes);
        if actual != manifest.artifact.hash {
            return Err(DerivedCorrupt::ArtifactHash {
                stored: manifest.artifact.hash,
                actual,
            });
        }
        Ok(Some(bytes))
    }

    fn manifest_path(&self, source: &ContentHash, producer: &str, kind: ProjectionKind) -> PathBuf {
        self.manifests_dir.join(format!(
            "{}.{}.{}.json",
            source.hex(),
            producer_slug(producer),
            kind.as_slug()
        ))
    }
}

/// Read a file, mapping not-found to `None`.
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Filesystem-safe producer id: unsafe characters fold to `-` and an
/// 8-hex blake3 tag of the raw id is appended so distinct producers
/// that sanitize identically never share a manifest file.
fn producer_slug(producer: &str) -> String {
    let safe: String = producer
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let tag: String = blake3::hash(producer.as_bytes()).as_bytes()[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{safe}-{tag}")
}

/// Milliseconds since the Unix epoch (provenance only, never ordered).
fn unix_epoch_ms() -> io::Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    u64::try_from(duration.as_millis()).map_err(io::Error::other)
}
