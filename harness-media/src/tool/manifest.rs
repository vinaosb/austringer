//! Declarative manifests for the seven plan tools (plan M15): name,
//! primary program, pinned version line, install hint, hard timeout,
//! extra env-deny patterns, per-op argv templates, and (later) the
//! pinned release asset. Every manifest is DISABLED by default - a tool
//! runs only through an explicit
//! [`ToolConfig`](crate::tool::ToolConfig) opt-in, and auto-install only
//! behind the install-enabled flag with SHA-256 verification.
//!
//! Version-pin honesty: a pin names a version line this workspace has
//! actually exercised (locally captured goldens) or the literal pending
//! marker until the tool's first real run (its adapter todo records the
//! run). Unverified versions are never claimed. The mocktool golden
//! capture commands and the pending-capture honesty table are committed
//! at `harness-test-support/golden/mocktool/capture.md`.

use std::collections::BTreeMap;

use harness_safety::supply::ReleaseAsset;

/// The op name every manifest carries: a cheap, side-effect-free argv
/// template identifying the installed build.
pub const OP_VERSION_CHECK: &str = "version-check";

/// One declared op invocation: the argv template plus an optional
/// per-op program override (gdal: `gdal_translate` vs `gdalinfo`).
/// Placeholders `{input}`, `{output}`, `{script}` are filled from the
/// caller's [`ToolCall::vars`](crate::tool::ToolCall::vars) map; an
/// unfilled placeholder is a typed spawn refusal, never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolOpTemplate {
    /// Op name (`version-check`, `probe`, `transcode`, ...).
    pub op: &'static str,
    /// Program when it differs from the spec's primary program.
    pub program: Option<&'static str>,
    /// argv template words.
    pub args: &'static [&'static str],
}

/// One external tool's declarative manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalToolSpec {
    /// Tool name (the registry key: `blender`, `ffmpeg`, ...).
    pub name: &'static str,
    /// Primary program on PATH (or a per-op override).
    pub program: &'static str,
    /// The pinned version line (see the module's honesty rule).
    pub version_pin: &'static str,
    /// Actionable install guidance surfaced in ToolUnavailable.
    pub install_hint: &'static str,
    /// Hard wall-clock timeout for one invocation.
    pub timeout_ms: u64,
    /// Extra env keys denied beyond `DEFAULT_REDACT_PATTERNS`.
    pub extra_env_deny: &'static [&'static str],
    /// Declared op argv templates.
    pub ops: &'static [ToolOpTemplate],
    /// Pinned release asset for the opt-in auto-install gate (None =
    /// operator installs; nothing is ever downloaded unpinned).
    pub release_asset: Option<ReleaseAsset>,
}

impl ExternalToolSpec {
    /// The template declared for `op`, if any.
    #[must_use]
    pub fn op_template(&self, op: &str) -> Option<&ToolOpTemplate> {
        self.ops.iter().find(|template| template.op == op)
    }

    /// The version-check argv (the universal first contact with a tool).
    #[must_use]
    pub fn version_check_args(&self) -> Option<Vec<&'static str>> {
        self.op_template(OP_VERSION_CHECK)
            .map(|template| template.args.to_vec())
    }
}

const VERSION_CHECK: ToolOpTemplate = ToolOpTemplate {
    op: OP_VERSION_CHECK,
    program: None,
    args: &["--version"],
};

const FFPROBE_VERSION_CHECK: ToolOpTemplate = ToolOpTemplate {
    op: OP_VERSION_CHECK,
    program: None,
    args: &["-version"],
};

// M17 ffmpeg lane: the FIXED invocations are manifest templates
// (machine-readable JSON census, keyframe thumbnail, lossless decode
// hash). The spec-driven transcode/encode/remux argv cannot be a
// static template - codec lanes, caps and trim come from the typed
// TranscodeSpec - so adapter::ffmpeg::policy builds those per call
// from its closed word table (test-pinned there; the filter-injection
// guard lives in the typed spec, never in free-form strings).
const FFPROBE_OP_PROBE_AV: ToolOpTemplate = ToolOpTemplate {
    op: "probe-av",
    program: None,
    args: &[
        "-v",
        "error",
        "-show_format",
        "-show_streams",
        "-of",
        "json",
        "{input}",
    ],
};

const FFMPEG_OP_THUMBNAIL: ToolOpTemplate = ToolOpTemplate {
    op: "thumbnail",
    program: None,
    args: &[
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        "0",
        "-i",
        "{input}",
        "-frames:v",
        "1",
        "{output}",
    ],
};

const FFMPEG_OP_PCM_MD5: ToolOpTemplate = ToolOpTemplate {
    op: "pcm-md5",
    program: None,
    args: &[
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        "{input}",
        "-map",
        "0:a",
        "-f",
        "md5",
        "-",
    ],
};

const GDALINFO_VERSION_CHECK: ToolOpTemplate = ToolOpTemplate {
    op: OP_VERSION_CHECK,
    program: Some("gdalinfo"),
    args: &["--version"],
};

// M20 step lane: freecadcmd runs the in-crate python templates - the
// builder script (argv tail = the output path; the post-export pass
// pins FILE_NAME to the determinism epoch) and the topology-dump
// script (argv tail = the input path; the census prints between the
// dump markers).
const FREECAD_OP_STEP_BUILD: ToolOpTemplate = ToolOpTemplate {
    op: "step-build",
    program: None,
    args: &["{script}", "{output}"],
};

const FREECAD_OP_TOPOLOGY_DUMP: ToolOpTemplate = ToolOpTemplate {
    op: "topology-dump",
    program: None,
    args: &["{script}", "{input}"],
};

const FREECAD_OPS: &[ToolOpTemplate] = &[
    VERSION_CHECK,
    FREECAD_OP_STEP_BUILD,
    FREECAD_OP_TOPOLOGY_DUMP,
];

// M21 georaster lane: the FIXED machine-readable census op is a
// manifest template. The spec-driven gdal_translate argv (EPSG,
// -a_ullr extent, COG flag) cannot be a static template - those words
// come from the typed GeoRasterSpec - so adapter::georaster::generate
// builds it per call from its closed word table (the M17 transcode
// pattern).
const GDAL_OP_INFO: ToolOpTemplate = ToolOpTemplate {
    op: "info",
    program: Some("gdalinfo"),
    args: &["-json", "{input}"],
};

const PENDING_PIN: &str = "pending-first-real-run";

fn spec(
    name: &'static str,
    program: &'static str,
    version_pin: &'static str,
    timeout_ms: u64,
    ops: &'static [ToolOpTemplate],
) -> ExternalToolSpec {
    ExternalToolSpec {
        name,
        program,
        version_pin,
        install_hint: "opt-in external tool, disabled by default: enable it in the media tool \
                       config and install it yourself (pinned auto-install ships with its \
                       release-asset pin)",
        timeout_ms,
        extra_env_deny: &[],
        ops,
        release_asset: None,
    }
}

// M16 blender lane: extract = headless scene-dump over the opened
// .blend (--factory-startup always - user prefs are never read);
// generate = builder script whose argv tail names the output path (the
// `--` separator is blender's pass-through to sys.argv).
const BLENDER_OP_SCENE_DUMP: ToolOpTemplate = ToolOpTemplate {
    op: "scene-dump",
    program: None,
    args: &[
        "--background",
        "--factory-startup",
        "{input}",
        "--python",
        "{script}",
    ],
};

const BLENDER_OP_SCENE_BUILD: ToolOpTemplate = ToolOpTemplate {
    op: "scene-build",
    program: None,
    args: &[
        "--background",
        "--factory-startup",
        "--python",
        "{script}",
        "--",
        "{output}",
    ],
};

const BLENDER_OPS: &[ToolOpTemplate] =
    &[VERSION_CHECK, BLENDER_OP_SCENE_DUMP, BLENDER_OP_SCENE_BUILD];
const FFMPEG_OPS: &[ToolOpTemplate] = &[VERSION_CHECK, FFMPEG_OP_THUMBNAIL, FFMPEG_OP_PCM_MD5];
const FFPROBE_OPS: &[ToolOpTemplate] = &[FFPROBE_VERSION_CHECK, FFPROBE_OP_PROBE_AV];
const WHISPER_OPS: &[ToolOpTemplate] = &[ToolOpTemplate {
    op: OP_VERSION_CHECK,
    program: None,
    args: &["--help"],
}];
const PANDOC_OPS: &[ToolOpTemplate] = &[VERSION_CHECK];
const GDAL_OPS: &[ToolOpTemplate] = &[GDALINFO_VERSION_CHECK, GDAL_OP_INFO];

/// The committed manifest table (all eight tools: ffmpeg and ffprobe
/// are distinct programs). No manifest carries a release asset yet - a
/// real pin requires a researched URL + SHA-256 pair, never an
/// invented digest.
pub fn manifests() -> &'static BTreeMap<String, ExternalToolSpec> {
    static TABLE: std::sync::LazyLock<BTreeMap<String, ExternalToolSpec>> =
        std::sync::LazyLock::new(|| {
            let mut table = BTreeMap::new();
            for entry in [
                spec("blender", "blender", PENDING_PIN, 300_000, BLENDER_OPS),
                // ffmpeg 8.1.1 exercised on the dev machine 2026-09-24
                // (golden captures; see golden/mocktool/capture.md).
                spec("ffmpeg", "ffmpeg", "8.1.1", 300_000, FFMPEG_OPS),
                spec("ffprobe", "ffprobe", "8.1.1", 30_000, FFPROBE_OPS),
                spec(
                    "whisper-cpp",
                    "whisper-cli",
                    PENDING_PIN,
                    600_000,
                    WHISPER_OPS,
                ),
                spec("libreoffice", "soffice", PENDING_PIN, 120_000, PANDOC_OPS),
                // pandoc 3.9.0.2 exercised on the dev machine 2026-09-24.
                spec("pandoc", "pandoc", "3.9.0.2", 60_000, PANDOC_OPS),
                spec(
                    "freecadcmd",
                    "freecadcmd",
                    PENDING_PIN,
                    180_000,
                    FREECAD_OPS,
                ),
                spec("gdal", "gdal_translate", PENDING_PIN, 180_000, GDAL_OPS),
            ] {
                table.insert(entry.name.to_owned(), entry);
            }
            table
        });
    &TABLE
}

/// Renders a template's argv with the caller's placeholder values; an
/// unfilled `{placeholder}` word is returned verbatim so the spawn
/// refuses loudly instead of guessing.
#[must_use]
pub fn render_args(template: &[&str], vars: &BTreeMap<String, String>) -> Vec<String> {
    template
        .iter()
        .map(|word| {
            if let Some(key) = word.strip_prefix('{').and_then(|k| k.strip_suffix('}')) {
                vars.get(key).cloned().unwrap_or_else(|| (*word).to_owned())
            } else {
                (*word).to_owned()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_fills_placeholders_and_leaves_unknown_words() {
        let mut vars = BTreeMap::new();
        vars.insert("input".to_owned(), "in.wav".to_owned());
        let rendered = render_args(&["-i", "{input}", "out.flac"], &vars);
        assert_eq!(rendered, vec!["-i", "in.wav", "out.flac"]);
        assert_eq!(
            render_args(&["{output}"], &vars),
            vec!["{output}"],
            "unfilled placeholders stay visible for the refusal"
        );
    }
}
