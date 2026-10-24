//! The subprocess op surface: probe census, transcode/remux, keyframe
//! thumbnail, and the lossless decode hash - every op staged inside
//! the executor's session temp (the sandbox writable root) and run
//! through the M15 [`ToolExecutor`] (redaction, kill-tree, timeout).
//!
//! Staging contract: the adapter publishes its staged input/output
//! paths in the child environment as `HARNESS_MEDIA_STAGE_IN` /
//! `HARNESS_MEDIA_STAGE_OUT` (absolute paths). Real ffmpeg ignores
//! them; the mocktool stand-in uses them for its `stagecopy` /
//! `stagegolden` modes (the pass-through remux echo + canned
//! thumbnail). Staged files are removed on both the success and
//! failure paths.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use harness_core::event::MediaStore;

use crate::error::MediaError;
use crate::executor::ProducerInfo;
use crate::extract::Extracted;
use crate::tool::{ToolCall, ToolError, ToolExecutor};

use super::argv::target_argv;
use super::census::AvProbe;
use super::policy::{FFMPEG_TOOL, FFPROBE_TOOL, TranscodeSpec};

static STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

const STAGE_IN_ENV: &str = "HARNESS_MEDIA_STAGE_IN";
const STAGE_OUT_ENV: &str = "HARNESS_MEDIA_STAGE_OUT";

/// The ffmpeg adapter handle: the tool executor plus the caller
/// environment every spawned child inherits (after redaction).
#[derive(Debug, Clone)]
pub struct Adapter<'a> {
    tools: &'a ToolExecutor,
    env: BTreeMap<String, String>,
}

/// One completed thumbnail: PNG bytes + producer disclosure.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredThumb {
    /// The PNG bytes (stored by the caller via [`MediaStore`]).
    pub bytes: Vec<u8>,
    /// The ffmpeg run that produced them.
    pub producer: ProducerInfo,
}

/// One completed transcode: artifact bytes + producer disclosure.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscodeArtifact {
    /// The transcoded/remuxed bytes.
    pub bytes: Vec<u8>,
    /// The ffmpeg run disclosure (the command line carries the clean
    /// codec flags verbatim).
    pub producer: ProducerInfo,
}

/// The full A/V extract result: the M1 [`Extracted`] contract plus the
/// per-tool producer disclosures and the stored thumbnail pair.
#[derive(Debug, Clone, PartialEq)]
pub struct AvExtract {
    /// Census tree + projection + thumbnail refs.
    pub extracted: Extracted,
    /// Producer disclosures for the artifacts above.
    pub producers: Vec<ProducerInfo>,
    /// The thumbnail pair when a video frame existed.
    pub thumbnail: Option<StoredThumb>,
}

impl<'a> Adapter<'a> {
    /// An adapter bound to a tool executor (empty child env).
    #[must_use]
    pub fn new(tools: &'a ToolExecutor) -> Self {
        Self {
            tools,
            env: BTreeMap::new(),
        }
    }

    /// The environment every spawned child inherits (redacted
    /// downstream by the executor).
    #[must_use]
    pub fn env(mut self, env: &'a BTreeMap<String, String>) -> Self {
        self.env = env.clone();
        self
    }

    fn stage_path(&self, tag: &str, extension: &str) -> PathBuf {
        self.tools.config().session_temp.join(format!(
            "m17-{tag}-{}-{}.{}",
            std::process::id(),
            STAGE_COUNTER.fetch_add(1, Ordering::Relaxed),
            extension,
        ))
    }

    fn stage_in(&self, bytes: &[u8], tool: &str) -> Result<PathBuf, MediaError> {
        let path = self.stage_path("in", "bin");
        std::fs::create_dir_all(&self.tools.config().session_temp)
            .map_err(|detail| spawn_error(tool, &format!("session temp: {detail}")))?;
        std::fs::write(&path, bytes).map_err(|detail| {
            spawn_error(tool, &format!("staging write {}: {detail}", path.display()))
        })?;
        Ok(path)
    }

    fn stage_env(&self, input: &Path, output: Option<&Path>) -> BTreeMap<String, String> {
        let mut env = self.env.clone();
        env.insert(STAGE_IN_ENV.to_owned(), input.display().to_string());
        if let Some(output) = output {
            env.insert(STAGE_OUT_ENV.to_owned(), output.display().to_string());
        }
        env
    }

    fn child_name(path: &Path) -> String {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn read_output(&self, path: &Path) -> Result<Vec<u8>, MediaError> {
        std::fs::read(path).map_err(|detail| {
            spawn_error(
                FFMPEG_TOOL,
                &format!("output artifact {}: {detail}", path.display()),
            )
        })
    }

    fn run_ffprobe(
        &self,
        input: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<(AvProbe, ProducerInfo), MediaError> {
        let mut call = ToolCall::new(FFPROBE_TOOL, "probe-av");
        call.vars
            .insert("input".to_owned(), Self::child_name(input));
        call.env = Some(env);
        let run = self.tools.run(&call)?;
        let census = AvProbe::parse(&String::from_utf8_lossy(&run.stdout))?;
        Ok((census, run.producer()))
    }

    /// The probe-deep lane: ffprobe's stream/format census.
    ///
    /// # Errors
    /// [`MediaError::ToolUnavailable`] when ffprobe is not
    /// enabled/installed; typed tool failures;
    /// [`MediaError::ParseFailure`] on non-JSON output.
    pub fn probe_av(&self, bytes: &[u8]) -> Result<AvProbe, MediaError> {
        let input = self.stage_in(bytes, FFPROBE_TOOL)?;
        let env = self.stage_env(&input, None);
        let census = self.run_ffprobe(&input, &env);
        cleanup(&input);
        census.map(|(probe, _)| probe)
    }

    /// The transcode/remux lane (the adapter's only generate-CLASS op:
    /// TRANSCODE, never from-nothing synthesis). The RED line is
    /// enforced here: stream-copy remux of RED input payloads refuses
    /// typed; re-encoding away from them stays legal.
    ///
    /// # Errors
    /// Typed refusals per the policy tables; tool failures; a failed
    /// run never yields a partial artifact.
    pub fn transcode(
        &self,
        bytes: &[u8],
        census: &AvProbe,
        spec: &TranscodeSpec,
    ) -> Result<TranscodeArtifact, MediaError> {
        if spec.target.is_remux()
            && let Some(red) = census.red_codec()
        {
            return Err(spec.red_remux_refusal(red));
        }
        let template = target_argv(spec, census.has_video(), census.video_width())?;
        let input = self.stage_in(bytes, FFMPEG_TOOL)?;
        let output = self.stage_path("out", spec.target.extension());
        let env = self.stage_env(&input, Some(&output));
        let argv: Vec<String> = template
            .iter()
            .map(|word| match word.as_str() {
                "{input}" => Self::child_name(&input),
                "{output}" => Self::child_name(&output),
                _ => word.clone(),
            })
            .collect();
        let mut call = ToolCall::new(FFMPEG_TOOL, "transcode").with_argv(argv);
        call.env = Some(&env);
        let run = self.tools.run(&call).map_err(MediaError::from);
        let artifact = run.and_then(|run| {
            self.read_output(&output).map(|bytes| TranscodeArtifact {
                bytes,
                producer: run.producer(),
            })
        });
        cleanup(&input);
        cleanup(&output);
        artifact
    }

    /// The keyframe-thumbnail lane: first video frame to PNG.
    /// Audio-only input is an honest `Ok(None)` - never a fabricated
    /// frame.
    ///
    /// # Errors
    /// Tool failures only (the census decides video presence).
    pub fn thumbnail(
        &self,
        bytes: &[u8],
        census: &AvProbe,
    ) -> Result<Option<StoredThumb>, MediaError> {
        if !census.has_video() {
            return Ok(None);
        }
        let input = self.stage_in(bytes, FFMPEG_TOOL)?;
        let output = self.stage_path("out", "png");
        let env = self.stage_env(&input, Some(&output));
        let mut call = ToolCall::new(FFMPEG_TOOL, "thumbnail");
        call.vars
            .insert("input".to_owned(), Self::child_name(&input));
        call.vars
            .insert("output".to_owned(), Self::child_name(&output));
        call.env = Some(&env);
        let run = self.tools.run(&call).map_err(MediaError::from);
        let thumb = run.and_then(|run| {
            self.read_output(&output)
                .map(|bytes| StoredThumb {
                    bytes,
                    producer: run.producer(),
                })
                .map(Some)
        });
        cleanup(&input);
        cleanup(&output);
        thumb
    }

    /// The lossless PCM hash hook (`-map 0:a -f md5 -`): decode-hash of
    /// the audio, `MD5=<hex>`.
    ///
    /// # Errors
    /// Tool failures; [`MediaError::ParseFailure`] when stdout carries
    /// no digest.
    pub fn pcm_md5(&self, bytes: &[u8]) -> Result<String, MediaError> {
        let input = self.stage_in(bytes, FFMPEG_TOOL)?;
        let env = self.stage_env(&input, None);
        let mut call = ToolCall::new(FFMPEG_TOOL, "pcm-md5");
        call.vars
            .insert("input".to_owned(), Self::child_name(&input));
        call.env = Some(&env);
        let run = self.tools.run(&call);
        cleanup(&input);
        let run = run?;
        let stdout = String::from_utf8_lossy(&run.stdout);
        let digest = stdout
            .split_once("MD5=")
            .map(|(_, tail)| tail.trim())
            .filter(|tail| !tail.is_empty())
            .ok_or(MediaError::ParseFailure)?;
        Ok(format!("MD5={digest}"))
    }

    /// The full extract: probe census + keyframe thumbnail stored in
    /// the caller's [`MediaStore`], every artifact carrying its
    /// producer disclosure.
    ///
    /// # Errors
    /// Composed from the probe/thumbnail lanes.
    pub fn extract_av(&self, bytes: &[u8], store: &MediaStore) -> Result<AvExtract, MediaError> {
        let input = self.stage_in(bytes, FFPROBE_TOOL)?;
        let env = self.stage_env(&input, None);
        let probed = self.run_ffprobe(&input, &env);
        cleanup(&input);
        let (census, probe_producer) = probed?;

        let mut producers = vec![probe_producer];
        let mut thumbnails = Vec::new();
        let mut stored = None;
        if let Some(thumb) = self.thumbnail(bytes, &census)? {
            let reference = store.store_media(&thumb.bytes).map_err(|detail| {
                spawn_error(FFMPEG_TOOL, &format!("media store write: {detail}"))
            })?;
            producers.push(thumb.producer.clone());
            thumbnails.push(reference);
            stored = Some(thumb);
        }
        let extracted = Extracted {
            tree: census.tree().clone(),
            text_projection: census.text_projection().to_owned(),
            thumbnails,
        };
        Ok(AvExtract {
            extracted,
            producers,
            thumbnail: stored,
        })
    }
}

fn spawn_error(tool: &str, detail: &str) -> MediaError {
    MediaError::from(ToolError::Spawn {
        tool: tool.to_owned(),
        detail: detail.to_owned(),
    })
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}
