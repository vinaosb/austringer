//! Persistent scratch stat-cache per session (the Cline PERF pattern) and
//! its 14-day reaper.
//!
//! The scratch file is a REAL git index (version 2, `DIRC`): every entry
//! carries `(mtime, size, blob-oid)`, so unchanged untracked files are
//! recognized by stat alone and never re-read or re-hashed. It lives at
//! `<workdir>/.harness/scratch/<session>/index` â€” harness state, never
//! inside the user's `.git`. Torn or corrupt scratch files are typed, and
//! the reaper removes scratch directories past the retention window.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::{SnapshotError, util};

pub const RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const INDEX_FILE: &str = "index";
const HEADER: &[u8; 4] = b"DIRC";
const VERSION: u32 = 2;
const ENTRY_FIXED: usize = 62;
const NAME_LEN_MASK: usize = 0xFFF;

/// The stat tuple the cache trusts: any drift invalidates the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScratchStat {
    pub mtime_secs: u64,
    pub mtime_nanos: u32,
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct ScratchIndex {
    entries: BTreeMap<String, (ScratchStat, String)>,
}

impl ScratchIndex {
    pub fn open(dir: &Path) -> Result<Self, SnapshotError> {
        let path = dir.join(INDEX_FILE);
        if !path.symlink_metadata().is_ok() {
            return Ok(Self::default());
        }
        let bytes = util::read_file(&path)?;
        Self::parse(&path, &bytes)
    }

    /// Reuse the cached blob oid when the stat still matches.
    pub fn lookup(&self, rel: &str, stat: &ScratchStat) -> Option<String> {
        let (cached, oid) = self.entries.get(rel)?;
        (cached == stat).then(|| oid.clone())
    }

    pub fn record(&mut self, rel: &str, stat: ScratchStat, oid_hex: &str) {
        self.entries
            .insert(rel.to_owned(), (stat, oid_hex.to_owned()));
    }

    pub fn save(&self, dir: &Path) -> Result<(), SnapshotError> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEADER);
        bytes.extend_from_slice(&VERSION.to_be_bytes());
        bytes.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for (rel, (stat, oid_hex)) in &self.entries {
            let oid = hex_to_oid(oid_hex);
            // ctime seconds (zero on Windows-style stat); u32-cast keeps
            // the entry layout exactly 62 fixed bytes.
            let ctime = u32::try_from(stat.mtime_secs).unwrap_or(u32::MAX);
            let mtime = u32::try_from(stat.mtime_secs).unwrap_or(u32::MAX);
            bytes.extend_from_slice(&ctime.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 4]); // ctime nanos
            bytes.extend_from_slice(&mtime.to_be_bytes());
            bytes.extend_from_slice(&stat.mtime_nanos.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 4]); // dev
            bytes.extend_from_slice(&[0u8; 4]); // ino
            bytes.extend_from_slice(&0o100644u32.to_be_bytes()); // mode
            bytes.extend_from_slice(&[0u8; 4]); // uid
            bytes.extend_from_slice(&[0u8; 4]); // gid
            bytes.extend_from_slice(&(stat.size as u32).to_be_bytes());
            bytes.extend_from_slice(&oid);
            let name = rel.as_bytes();
            let stored_len = name.len().min(NAME_LEN_MASK);
            bytes.extend_from_slice(&(stored_len as u16).to_be_bytes());
            bytes.extend_from_slice(name);
            let pad = 8 - (ENTRY_FIXED + name.len()) % 8;
            bytes.extend(std::iter::repeat_n(0u8, pad.max(1)));
        }
        let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
        hasher.update(&bytes);
        let trailer = hasher
            .try_finalize()
            .map_err(|_| SnapshotError::ScratchIndex {
                path: dir.join(INDEX_FILE),
                reason: "digest",
            })?;
        bytes.extend_from_slice(trailer.as_bytes());
        util::create_dir_all(dir)?;
        util::write_file(&dir.join(INDEX_FILE), &bytes)
    }

    fn parse(path: &Path, bytes: &[u8]) -> Result<Self, SnapshotError> {
        let torn = |reason: &'static str| SnapshotError::ScratchIndex {
            path: path.to_path_buf(),
            reason,
        };
        if bytes.len() < 12 || &bytes[0..4] != HEADER {
            return Err(torn("bad magic"));
        }
        let version = u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| torn("header"))?);
        if version != VERSION {
            return Err(torn("unsupported version"));
        }
        let count =
            u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| torn("header"))?) as usize;
        let mut cursor = 12usize;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            if cursor + ENTRY_FIXED > bytes.len() {
                return Err(torn("torn entry"));
            }
            let mtime_secs = u64::from(u32::from_be_bytes(
                bytes[cursor + 8..cursor + 12]
                    .try_into()
                    .map_err(|_| torn("mtime"))?,
            ));
            let mtime_nanos = u32::from_be_bytes(
                bytes[cursor + 12..cursor + 16]
                    .try_into()
                    .map_err(|_| torn("mtime"))?,
            );
            let size = u64::from(u32::from_be_bytes(
                bytes[cursor + 36..cursor + 40]
                    .try_into()
                    .map_err(|_| torn("size"))?,
            ));
            let oid = bytes[cursor + 40..cursor + 60].to_vec();
            let flags = u16::from_be_bytes(
                bytes[cursor + 60..cursor + 62]
                    .try_into()
                    .map_err(|_| torn("flags"))?,
            );
            let declared = (flags as usize) & NAME_LEN_MASK;
            let name_end = if declared == NAME_LEN_MASK {
                bytes[cursor + ENTRY_FIXED..]
                    .iter()
                    .position(|b| *b == 0)
                    .map(|at| cursor + ENTRY_FIXED + at)
            } else {
                Some(cursor + ENTRY_FIXED + declared)
            };
            let Some(name_end) = name_end.filter(|at| *at < bytes.len()) else {
                return Err(torn("torn name"));
            };
            let rel = String::from_utf8_lossy(&bytes[cursor + ENTRY_FIXED..name_end]).into_owned();
            let consumed = ENTRY_FIXED + (name_end - cursor - ENTRY_FIXED);
            cursor += consumed + (8 - consumed % 8).max(1);
            entries.insert(
                rel,
                (
                    ScratchStat {
                        mtime_secs,
                        mtime_nanos,
                        size,
                    },
                    oid.iter().map(|b| format!("{b:02x}")).collect(),
                ),
            );
        }
        Ok(Self { entries })
    }
}

fn hex_to_oid(hex: &str) -> [u8; 20] {
    let mut oid = [0u8; 20];
    for (i, slot) in oid.iter_mut().enumerate() {
        if let Some(Ok(byte)) = hex
            .get(2 * i..2 * i + 2)
            .map(|pair| u8::from_str_radix(pair, 16))
        {
            *slot = byte;
        }
    }
    oid
}

/// Remove scratch directories whose index mtime is older than `retention`
/// as of `now`. The production call uses [`RETENTION`].
pub fn reap_scratch(
    root: &Path,
    now: SystemTime,
    retention: Duration,
) -> Result<usize, SnapshotError> {
    let Ok(children) = util::read_dir_sorted(root) else {
        return Ok(0);
    };
    let mut reaped = 0;
    for child in children {
        let mtime = std::fs::metadata(util::extended(&child))
            .and_then(|meta| meta.modified())
            .map_err(|source| SnapshotError::io("scratch stat", source))?;
        let expired = now
            .duration_since(mtime)
            .map(|age| age >= retention)
            .unwrap_or(false);
        if expired {
            util::remove_dir_all(&child)?;
            reaped += 1;
        }
    }
    Ok(reaped)
}
