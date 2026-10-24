//! The fixed argv template set (the filter-injection guard's
//! enforcement point): typed spec + input census to ffmpeg argv
//! words. `{input}`/`{output}` are staging tokens the op seam
//! substitutes; every other word is a fixed literal from the digest
//! section 4 clean table or a validated number. No spec STRING ever
//! reaches argv - the universe of emittable words is test-pinned over
//! every target x cap combination.

use crate::error::MediaError;

use super::policy::TranscodeSpec;

/// Build the ffmpeg argv for one typed transcode spec.
///
/// # Errors
/// [`MediaError::UnsupportedOperation`] when the target expects video
/// and `has_video` is false.
pub fn target_argv(
    spec: &TranscodeSpec,
    has_video: bool,
    src_width: Option<u32>,
) -> Result<Vec<String>, MediaError> {
    let target = spec.target;
    if target.wants_video() && !has_video {
        return Err(spec.no_video_refusal());
    }
    let mut argv: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error"]
        .iter()
        .map(ToString::to_string)
        .collect();
    if let Some(trim) = spec.trim {
        // -ss + -t (duration), not -ss + -to: duration semantics are
        // identical whether ffmpeg reads the pair as input or output
        // options (input-seek stays the fast path).
        argv.push("-ss".into());
        argv.push(trim.start_secs.to_string());
        argv.push("-t".into());
        argv.push((trim.end_secs - trim.start_secs).to_string());
    }
    argv.push("-i".into());
    argv.push("{input}".into());
    if target.is_remux() {
        argv.extend(["-c", "copy"].iter().map(ToString::to_string));
        argv.push("{output}".into());
        return Ok(argv);
    }
    let (vcodec, acodec) = target.encoder_flags();
    let video_lane = target.wants_video();
    if let Some(encoder) = vcodec {
        argv.push("-c:v".into());
        argv.push(encoder.into());
    } else if !video_lane {
        argv.push("-vn".into());
    }
    if let Some(encoder) = acodec {
        argv.push("-c:a".into());
        argv.push(encoder.into());
    }
    if video_lane {
        if let Some(max_width) = spec.caps.max_width
            && src_width.is_none_or(|width| width > max_width)
        {
            argv.push("-vf".into());
            argv.push(format!("scale={max_width}:-2"));
        }
        if let Some(max_fps) = spec.caps.max_fps {
            argv.push("-r".into());
            argv.push(max_fps.to_string());
        }
    }
    if let Some(kbps) = spec.caps.max_audio_kbps {
        argv.push("-b:a".into());
        argv.push(format!("{kbps}k"));
    }
    if target == super::policy::Target::WebpLossy {
        argv.extend(
            ["-frames:v", "1", "-quality", "80"]
                .iter()
                .map(ToString::to_string),
        );
    }
    argv.push("{output}".into());
    Ok(argv)
}
