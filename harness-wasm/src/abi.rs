//! The versioned interface ladder and the ABI table (the "simplified WIT",
//! per plan: typed version negotiation, not full WIT tooling).
//!
//! # Interface ladder
//!
//! A plugin DECLARES its interface version in the manifest
//! (`"interface": N`). The host runs a RANGE, [`INTERFACE_MIN`] through
//! [`INTERFACE_MAX`]:
//!
//! | Plugin declares | Host action                                              |
//! | --------------- | -------------------------------------------------------- |
//! | within range    | dispatch through that interface's ABI (see [`Interface`]) |
//! | older than min  | typed `InterfaceTooOld` - guidance: rebuild the plugin    |
//! | newer than max  | typed `InterfaceTooNew` - guidance: upgrade the host      |
//!
//! This is Zed's 10-WIT-versions lesson (research §23) in miniature: OLD
//! interfaces keep running across host upgrades because the host never
//! removes a dispatch arm - it only raises [`INTERFACE_MIN`] deliberately,
//! with a release note. Adding interface 2 means: bump [`INTERFACE_MAX`],
//! add an [`Interface::V2`] arm, and teach [`negotiate`] to route to it.
//!
//! # ABI table, interface 1
//!
//! The guest exports:
//!
//! | Export       | Signature                     | Meaning                          |
//! | ------------ | ----------------------------- | -------------------------------- |
//! | `memory`     | linear memory (>= 1 page)     | all data exchange regions        |
//! | `tool_call`  | `(tool_id: i32, len: i32) -> i32` | run tool `tool_id` on input |
//!
//! Memory regions (fixed offsets, all within the first 64 KiB page):
//!
//! | Region     | Offset             | Capacity             | Direction       |
//! | ---------- | ------------------ | -------------------- | --------------- |
//! | scratch    | `0`                | `4096`               | guest-private   |
//! | input      | [`INPUT_OFFSET`]   | [`INPUT_CAPACITY`]   | host writes     |
//! | output     | [`OUTPUT_OFFSET`]  | [`OUTPUT_CAPACITY`]  | guest writes    |
//! | manifest   | [`MANIFEST_OFFSET`]| [`MANIFEST_CAPACITY`]| build-time, NUL-terminated JSON |
//!
//! `tool_call` returns the output byte count (>= 0) or a negative
//! guest-defined error code. The host imports, under module
//! [`HOST_MODULE`]:
//!
//! | Import       | Signature                                    | Purpose |
//! | ------------ | -------------------------------------------- | ------- |
//! | `log`        | `(level: i32, ptr: i32, len: i32) -> ()`     | bounded, lossy-UTF-8 log line (0 info, 1 warn, 2 error) |
//! | `read_file`  | `(path, plen, buf, cap: i32) -> i32`         | read a file inside a preopen scope into `buf`; returns bytes or a negative code |
//! | `write_file` | `(path, plen, data, dlen: i32) -> i32`      | write bytes inside an RW preopen scope; 0 or a negative code |
//!
//! Host-fn paths are VIRTUAL (`"<scope>/<relative>"`, e.g.
//! `data/note.txt`); resolution canonicalizes within the granted scope
//! root only - `..`, absolute paths, and symlink escapes are denied.
//! Negative host codes: -1 unknown scope, -2 path escape, -3 read-only
//! scope, -4 not-found/IO, -5 too large, -6 bad guest pointer.

/// Oldest interface this host runs.
pub const INTERFACE_MIN: u8 = 1;

/// Newest interface this host runs.
pub const INTERFACE_MAX: u8 = 1;

/// Offset of the host-written tool input region.
pub const INPUT_OFFSET: usize = 4096;

/// Capacity of the tool input region.
pub const INPUT_CAPACITY: usize = 4096;

/// Offset of the guest-written tool output region.
pub const OUTPUT_OFFSET: usize = 8192;

/// Capacity of the tool output region.
pub const OUTPUT_CAPACITY: usize = 4096;

/// Offset of the NUL-terminated manifest region.
pub const MANIFEST_OFFSET: usize = 16384;

/// Capacity of the manifest region.
pub const MANIFEST_CAPACITY: usize = 8192;

/// The host-function import module name.
pub const HOST_MODULE: &str = "harness";

/// A negotiated interface: the dispatch arm the host runs a plugin through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    /// Interface 1 - the ABI table in this module's docs.
    V1,
}

impl Interface {
    /// The wire version number of this interface.
    pub fn version(self) -> u8 {
        match self {
            Self::V1 => 1,
        }
    }
}

/// Version negotiation failures, each carrying user guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InterfaceError {
    /// Declared version is below the host's minimum.
    #[error(
        "interface {declared} is older than the minimum supported ({min}); rebuild the plugin against interface {min}"
    )]
    TooOld {
        /// Declared version.
        declared: u8,
        /// Host minimum.
        min: u8,
    },
    /// Declared version is above the host's maximum.
    #[error(
        "interface {declared} is newer than this host supports (max {max}); upgrade the host to run this plugin"
    )]
    TooNew {
        /// Declared version.
        declared: u8,
        /// Host maximum.
        max: u8,
    },
}

/// Negotiate a plugin's declared interface against the host's supported
/// range. Older interfaces stay runnable across host upgrades by design;
/// only the deliberate raising of [`INTERFACE_MIN`] retires one.
pub fn negotiate(declared: u8) -> Result<Interface, InterfaceError> {
    if declared < INTERFACE_MIN {
        return Err(InterfaceError::TooOld {
            declared,
            min: INTERFACE_MIN,
        });
    }
    if declared > INTERFACE_MAX {
        return Err(InterfaceError::TooNew {
            declared,
            max: INTERFACE_MAX,
        });
    }
    // The dispatch ladder: every supported version routes somewhere, and
    // new versions are added as new arms without touching old ones.
    Ok(Interface::V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_range_versions_negotiate() {
        assert_eq!(negotiate(INTERFACE_MIN), Ok(Interface::V1));
        assert_eq!(negotiate(INTERFACE_MAX), Ok(Interface::V1));
    }

    #[test]
    fn too_old_carries_rebuild_guidance() -> Result<(), Box<dyn std::error::Error>> {
        match negotiate(INTERFACE_MIN - 1) {
            Err(InterfaceError::TooOld {
                declared: 0,
                min: 1,
            }) => {}
            other => return Err(format!("expected TooOld(0, 1), got {other:?}").into()),
        }
        let msg = negotiate(INTERFACE_MIN - 1).err().map(|e| e.to_string());
        assert!(
            msg.is_some_and(|m| m.contains("rebuild")),
            "guidance must say rebuild"
        );
        Ok(())
    }

    #[test]
    fn too_new_carries_upgrade_guidance() -> Result<(), Box<dyn std::error::Error>> {
        match negotiate(INTERFACE_MAX + 1) {
            Err(InterfaceError::TooNew {
                declared: 2,
                max: 1,
            }) => {}
            other => return Err(format!("expected TooNew(2, 1), got {other:?}").into()),
        }
        let msg = negotiate(INTERFACE_MAX + 1).err().map(|e| e.to_string());
        assert!(
            msg.is_some_and(|m| m.contains("upgrade")),
            "guidance must say upgrade"
        );
        Ok(())
    }

    // Region-layout invariants, enforced at compile time: the input, output,
    // and manifest regions are disjoint and all fit in the first 64 KiB page.
    const _: () = assert!(INPUT_OFFSET + INPUT_CAPACITY <= OUTPUT_OFFSET);
    const _: () = assert!(OUTPUT_OFFSET + OUTPUT_CAPACITY <= MANIFEST_OFFSET);
    const _: () = assert!(MANIFEST_OFFSET + MANIFEST_CAPACITY <= 65536);
}
