//! T29 roots confinement: the CLIENT enforces server root boundaries by
//! refusing to send tool calls whose arguments touch paths outside every
//! declared root (root-escape attempts are denied before any bytes reach
//! the server).

use harness_mcp::config::ServerEntry;
use harness_mcp::config::TransportSpec;
use harness_mcp::error::McpClientError;
use harness_mcp::roots::Roots;
use serde_json::json;
use std::path::PathBuf;

fn roots(list: &[&str]) -> Roots {
    Roots::new(list.iter().map(PathBuf::from).collect())
}

#[cfg(windows)]
fn win(p: &str) -> String {
    p.to_owned()
}
#[cfg(not(windows))]
fn win(p: &str) -> String {
    // Map windows drive paths to rooted posix equivalents so the
    // absolute-vs-relative classification holds on unix too: the repo
    // drive keeps a stable /repo prefix, other drives land under /win.
    if let Some(rest) = p.strip_prefix(r"C:\repo")
        && (rest.is_empty() || rest.starts_with('\\'))
    {
        return format!("/repo{}", rest.replace('\\', "/"));
    }
    let bytes = p.as_bytes();
    if bytes.len() > 2 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
        let drive = p[..1].to_ascii_lowercase();
        return format!("/win/{drive}{}", p[2..].replace('\\', "/"));
    }
    p.replace('\\', "/")
}

#[test]
fn absolute_path_inside_root_is_allowed() {
    let guard = roots(&[&win("C:\\repo")]);
    let args = json!({ "path": win("C:\\repo\\src\\main.rs") });
    assert_eq!(guard.check_arguments("read_file", &args), Ok(()));
}

#[test]
fn absolute_path_outside_root_is_denied_typed() {
    let guard = roots(&[&win("C:\\repo")]);
    let args = json!({ "path": win("C:\\Windows\\system32\\config\\sam") });
    assert_eq!(
        guard.check_arguments("read_file", &args),
        Err(McpClientError::RootsEscape {
            tool: "read_file".to_owned(),
            path: win("C:\\Windows\\system32\\config\\sam"),
        })
    );
}

#[test]
fn dotdot_traversal_escaping_every_root_is_denied() {
    let guard = roots(&[&win("C:\\repo\\sub")]);
    let args = json!({ "path": win("C:\\repo\\sub\\..\\..\\secrets.env") });
    assert!(matches!(
        guard.check_arguments("read_file", &args),
        Err(McpClientError::RootsEscape { .. })
    ));
}

#[test]
fn dotdot_staying_inside_some_root_is_allowed() {
    let guard = roots(&[&win("C:\\repo")]);
    let args = json!({ "path": win("C:\\repo\\sub\\..\\other.rs") });
    assert_eq!(guard.check_arguments("read_file", &args), Ok(()));
}

#[test]
fn nested_json_is_scanned_recursively() {
    let guard = roots(&[&win("C:\\repo")]);
    let args = json!({
        "options": { "files": [win("C:\\repo\\a.rs"), win("D:\\evil\\b.rs")], "depth": 3 }
    });
    assert!(matches!(
        guard.check_arguments("read_many", &args),
        Err(McpClientError::RootsEscape { .. })
    ));
}

#[test]
fn no_declared_roots_denies_every_absolute_path() {
    let guard = roots(&[]);
    let args = json!({ "path": win("C:\\anywhere\\file.txt") });
    assert!(matches!(
        guard.check_arguments("read_file", &args),
        Err(McpClientError::RootsEscape { .. })
    ));
}

#[test]
fn plain_relative_paths_pass() {
    let guard = roots(&[&win("C:\\repo")]);
    let args = json!({ "path": "src/main.rs", "note": "just a sentence with a colon: no" });
    assert_eq!(guard.check_arguments("read_file", &args), Ok(()));
}

#[test]
fn unicode_paths_confinement_works() {
    let guard = roots(&[&win("C:\\repo\\ファイル")]);
    let inside = json!({ "path": win("C:\\repo\\ファイル\\読み取り.txt") });
    let outside = json!({ "path": win("C:\\別の所\\機密.txt") });
    assert_eq!(guard.check_arguments("read_file", &inside), Ok(()));
    assert!(matches!(
        guard.check_arguments("read_file", &outside),
        Err(McpClientError::RootsEscape { .. })
    ));
}

#[test]
fn entry_roots_feed_the_guard() {
    let mut entry = ServerEntry::new(
        "fs".to_owned(),
        TransportSpec::Stdio {
            command: "npx".to_owned(),
            args: vec![],
        },
    );
    entry.roots.push(PathBuf::from(win("C:\\repo")));
    let guard = Roots::from_entry(&entry);
    let inside = json!({ "path": win("C:\\repo\\x.rs") });
    assert_eq!(guard.check_arguments("read_file", &inside), Ok(()));
}
