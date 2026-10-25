//! # ffmpeg/ffprobe A/V adapter (plan M17, subprocess tier)
//!
//! The subprocess lane every pure-tier redirect points at: opus/mp3/
//! vorbis encode and opus decode (M14), lossy/animated webp encode
//! (M28). Everything runs through the M15 [`ToolExecutor`] - opt-in,
//! disabled by default, sandboxed to the session temp, redacted env,
//! kill-tree timeout - and every artifact carries its
//! [`ProducerInfo`] disclosure.
//!
//! ## Vocabulary (TRANSCODE, not generation)
//!
//! There is no from-nothing synthesis here. The generate-CLASS ops are
//! transcode/remux over an input census:
//! [`Adapter::probe_av`] (ffprobe JSON census), [`Adapter::transcode`]
//! (typed GREEN targets + caps + trim), [`Adapter::thumbnail`]
//! (keyframe PNG), [`Adapter::pcm_md5`] (lossless decode hash),
//! [`Adapter::extract_av`] (census + thumbnail into the MediaStore),
//! and [`verify_transcode`] (probe-diff on declared targets).
//!
//! ## Licensing anchor (digest sections 2+4, standing constraint 7)
//!
//! The [`policy`](self::policy) module is law: encode only to GREEN
//! targets, refuse RED outputs and RED stream-copy remuxes, decode
//! away from RED inputs freely. H.264/H.265/AAC are never emitted -
//! `libfdk_aac` makes a build unredistributable and native `aac` is
//! content-encumbered. MP4-container remux is the deferred lane.
//!
//! ## Hermetic contract
//!
//! Tests substitute mocktool per tool (distinct child entries per tool
//! name); canned outputs are REAL captured goldens under
//! `austringer-test-support/golden/mocktool/` (see its capture.md). The
//! real-tool tier is `#[ignore]` behind `FFMPEG_BIN` and never runs in
//! CI.

mod argv;
mod census;
mod ops;
mod policy;
mod verify;

pub use argv::target_argv;
pub use census::{AvProbe, MAX_CENSUS_STREAMS, StreamCensus};
pub use ops::{Adapter, AvExtract, StoredThumb, TranscodeArtifact};
pub use policy::{Caps, FFMPEG_TOOL, FFPROBE_TOOL, Target, TranscodeSpec, Trim};
pub use verify::verify_transcode;
