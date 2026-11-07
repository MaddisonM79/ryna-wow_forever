//! The binary's `parse` against the committed fixture: valid section 2 JSON on stdout.

use std::process::Command;

fn fixture() -> String {
    format!(
        "{}/../../cores/wow/fixtures/Ryna.lua",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn parse_prints_the_snapshot() {
    let out = Command::new(env!("CARGO_BIN_EXE_ryna"))
        .args(["parse", &fixture(), "--account", "FAKEACCOUNT1"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema"], 1);
    assert_eq!(v["flavor"], "wowf");
    assert_eq!(v["account"]["label"], "UNT1");
    assert_eq!(v["account"]["key"].as_str().unwrap().len(), 16);
    assert_eq!(v["characters"].as_array().unwrap().len(), 2);
    assert!(v.get("client").is_none() && v.get("settings").is_none());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("client: interface 11600"), "{stderr}");
}

#[test]
fn parse_raw_prints_the_lua_as_json() {
    let out = Command::new(env!("CARGO_BIN_EXE_ryna"))
        .args(["parse", &fixture(), "--raw"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["RynaDB"]["client"]["interface"], 11600);
    assert!(v["RynaDB"]["characters"]["Player-91-0F00A001"].is_object());
}

#[test]
fn parse_refuses_a_bad_file_with_a_position() {
    let dir = std::env::temp_dir().join(format!("ryna-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("Ryna.lua");
    std::fs::write(&f, "RynaDB = {\n  schema = 1,\n  @\n}").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ryna"))
        .args(["parse", f.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("line 3, column 3"));
}
