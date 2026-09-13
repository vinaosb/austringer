//! Manifest parsing and validation (pure types, no engine needed - runs in
//! every build configuration).

use harness_wasm::manifest::{Capability, ManifestError, PreopenMode, parse};

type R = Result<(), Box<dyn std::error::Error>>;

const GOOD: &str = "{\"name\":\"p\",\"interface\":1,\"capabilities\":[\"download\"],\
    \"preopens\":[{\"name\":\"d\",\"mode\":\"rw\"}],\
    \"tools\":[{\"name\":\"t\",\"description\":\"d\",\"input_schema\":{\"type\":\"object\"}}]}";

#[test]
fn parses_and_validates_a_full_manifest() -> R {
    let m = parse(GOOD.as_bytes())?;
    assert_eq!(m.name, "p");
    assert_eq!(m.interface, 1);
    assert_eq!(m.capabilities, vec![Capability::Download]);
    assert_eq!(m.preopens[0].mode, PreopenMode::ReadWrite);
    assert_eq!(m.tool_id("t"), Some(0));
    assert_eq!(m.tool_id("missing"), None);
    Ok(())
}

#[test]
fn unknown_capability_is_typed() {
    let text = GOOD.replace("download", "root-access");
    assert!(
        matches!(
            parse(text.as_bytes()),
            Err(ManifestError::UnknownCapability { .. })
        ),
        "unknown capability must be typed"
    );
}

#[test]
fn unknown_mode_is_typed() {
    let text = GOOD.replace("\"rw\"", "\"rx\"");
    assert!(
        matches!(
            parse(text.as_bytes()),
            Err(ManifestError::UnknownMode { .. })
        ),
        "unknown mode must be typed"
    );
}

#[test]
fn duplicate_tool_is_typed() {
    let tool = "{\"name\":\"t\",\"description\":\"d\",\"input_schema\":{\"type\":\"object\"}}";
    let text = format!("{{\"name\":\"p\",\"interface\":1,\"tools\":[{tool},{tool}]}}");
    assert!(
        matches!(
            parse(text.as_bytes()),
            Err(ManifestError::DuplicateTool { .. })
        ),
        "duplicate tool must be typed"
    );
}

#[test]
fn schema_must_be_object() {
    let text = GOOD.replace("{\"type\":\"object\"}", "\"string\"");
    assert!(
        matches!(
            parse(text.as_bytes()),
            Err(ManifestError::SchemaNotObject { .. })
        ),
        "non-object schema must be typed"
    );
}

#[test]
fn no_tools_is_typed() {
    let text = "{\"name\":\"p\",\"interface\":1,\"tools\":[]}";
    assert!(
        matches!(parse(text.as_bytes()), Err(ManifestError::NoTools)),
        "tool-less plugin must be typed"
    );
}

#[test]
fn bad_json_and_bad_utf8_are_typed() {
    assert!(matches!(
        parse(b"{oops"),
        Err(ManifestError::InvalidJson { .. })
    ));
    assert!(matches!(parse(&[0xff, 0xfe]), Err(ManifestError::NotUtf8)));
}
