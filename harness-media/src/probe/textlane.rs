//! Text lanes, in fixed order: the XML root lane (svg/kml), the JSON
//! lane (gltf partial sniff, geojson, json/jsonl), then leading-content
//! heuristics (html > toml > csv > markdown when several fire). All
//! lanes are bounded to a leading window; the JSON full parse is
//! additionally capped in total size. html is NEVER Certain here - it is
//! a heuristic-lane format by the acceptance contract (only svg/kml have
//! structural roots).

use crate::format::FormatId;
use crate::probe::{find_sub, hit, likely, unknown, ProbeResult};
use serde_json::Value;

const WINDOW: usize = 8 * 1024;
const JSON_MAX: usize = 512 * 1024;
const SVG_NS: &str = "http://www.w3.org/2000/svg";
const KML_NS: &str = "http://www.opengis.net/kml/2.2";

pub(super) fn sniff(bytes: &[u8], hint: Option<FormatId>) -> Option<ProbeResult> {
    if let Some(result) = xml_root(bytes, hint) {
        return Some(result);
    }
    if let Some(result) = json_lane(bytes) {
        return Some(result);
    }
    heuristics(bytes, hint)
}

/// Root-element + namespace sniff for the two structural XML formats.
/// Skips a BOM, whitespace, prolog, comments, and DOCTYPE, then reads the
/// root name (namespace prefix tolerated). An html root deliberately
/// falls through to the heuristics lane.
fn xml_root(bytes: &[u8], hint: Option<FormatId>) -> Option<ProbeResult> {
    let text = std::str::from_utf8(bytes.get(..WINDOW).unwrap_or(bytes)).ok()?;
    let mut cursor = text.strip_prefix('\u{feff}').unwrap_or(text).trim_start();
    while cursor.starts_with('<') {
        let tail = if let Some(t) = cursor.strip_prefix("<?") {
            t.find("?>").map(|at| &t[at + 2..])
        } else if let Some(t) = cursor.strip_prefix("<!--") {
            t.find("-->").map(|at| &t[at + 3..])
        } else if let Some(t) = cursor.strip_prefix("<!DOCTYPE") {
            t.find('>').map(|at| &t[at + 1..])
        } else {
            break;
        }?;
        cursor = tail.trim_start();
    }
    let open = cursor.strip_prefix('<')?;
    let name_end = open.find(['>', ' ', '\t', '\r', '\n', '/'])?;
    let root = &open[..name_end];
    let local = root.rsplit(':').next().unwrap_or(root);
    if local == "svg" {
        return Some(hit(FormatId::Svg, "svg root element".into()));
    }
    let tag = &cursor[..cursor.find('>')?];
    if local == "kml" {
        if tag.contains(KML_NS) {
            return Some(hit(FormatId::Kml, "kml root + 2.2 namespace".into()));
        }
        return match hint {
            Some(FormatId::Kml) => Some(likely(
                FormatId::Kml,
                "kml root without the 2.2 namespace; name hint".into(),
            )),
            _ => Some(unknown("kml-like root without the 2.2 namespace")),
        };
    }
    if tag.contains(SVG_NS) {
        return Some(hit(FormatId::Svg, "svg namespace on the root element".into()));
    }
    None
}

/// glTF partial sniff first (allocation-free leading-window byte scan for
/// a quoted asset + version key, the plan's exact wording), then the
/// bounded full parse for geojson/json, then the JSONL line check.
fn json_lane(bytes: &[u8]) -> Option<ProbeResult> {
    let head = bytes.get(..WINDOW).unwrap_or(bytes);
    let starts_object = head
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .is_some_and(|at| head.get(at) == Some(&b'{'));
    if starts_object
        && find_sub(head, b"\"asset\"").is_some()
        && find_sub(head, b"\"version\"").is_some()
    {
        return Some(hit(
            FormatId::Gltf,
            "json with asset.version (partial sniff)".into(),
        ));
    }
    if bytes.len() > JSON_MAX {
        return None;
    }
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return jsonl_lane(bytes);
    };
    if let Value::Object(map) = &value
        && matches!(map.get("type"), Some(Value::String(kind)) if kind == "Feature" || kind == "FeatureCollection")
    {
        return Some(likely(
            FormatId::Geojson,
            "json with a Feature/FeatureCollection type".into(),
        ));
    }
    Some(likely(FormatId::Json, "parses as JSON".into()))
}

fn jsonl_lane(bytes: &[u8]) -> Option<ProbeResult> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let first = lines.next()?;
    if lines.all(|line| serde_json::from_str::<Value>(line).is_ok())
        && serde_json::from_str::<Value>(first).is_ok()
    {
        return Some(likely(
            FormatId::Json,
            "JSONL: every line parses as JSON".into(),
        ));
    }
    None
}

/// Leading-window heuristics. Priority when several fire is fixed
/// (html > toml > csv > markdown); the hint may only pick among the
/// candidates that actually fired.
fn heuristics(bytes: &[u8], hint: Option<FormatId>) -> Option<ProbeResult> {
    let window = std::str::from_utf8(bytes.get(..WINDOW).unwrap_or(bytes)).ok()?;
    if window.trim().is_empty() {
        return None;
    }
    let lower = window.to_ascii_lowercase();
    let fired = [
        (
            ["<html", "<!doctype html", "<head", "<body"]
                .iter()
                .any(|token| lower.contains(token)),
            FormatId::Html,
            "html tags in leading content",
        ),
        (
            toml_signal(window),
            FormatId::Toml,
            "toml section/key-value structure",
        ),
        (
            csv_signal(window),
            FormatId::Csv,
            "consistent delimiter-separated fields",
        ),
        (
            markdown_signal(window),
            FormatId::Markdown,
            "markdown heading/list/emphasis markers",
        ),
    ];
    if let Some(hinted) = hint
        && let Some(&(_, format, note)) =
            fired.iter().find(|(on, format, _)| *on && *format == hinted)
    {
        return Some(likely(format, note.into()));
    }
    let &(_, format, note) = fired.iter().find(|(on, _, _)| *on)?;
    Some(likely(format, note.into()))
}

fn toml_signal(window: &str) -> bool {
    window.lines().take(64).any(|line| {
        let trimmed = line.trim();
        (trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2)
            || trimmed.split_once('=').is_some_and(|(key, value)| {
                let key = key.trim();
                !key.is_empty()
                    && !value.trim().is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
    })
}

fn csv_signal(window: &str) -> bool {
    let lines: Vec<&str> = window
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(20)
        .collect();
    if lines.len() < 2 {
        return false;
    }
    b",;\t".iter().any(|&delimiter| {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| line.bytes().filter(|&b| b == delimiter).count())
            .collect();
        counts.first().is_some_and(|&first| first >= 1 && counts.iter().all(|&c| c == first))
    })
}

fn markdown_signal(window: &str) -> bool {
    window.lines().take(64).any(|line| {
        let trimmed = line.trim_start();
        let heading = trimmed.starts_with('#') && trimmed.trim_start_matches('#').starts_with(' ');
        let bullet = trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed.starts_with("```");
        heading || bullet || line.contains("**") || line.contains("](")
    })
}
