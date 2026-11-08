//! `ryna settings list|diff|copy` against a fake game tree in a temporary folder (never a real
//! WTF folder). The local commands touch no keychain and no network.

use std::path::Path;
use std::process::{Command, Output};

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A `_forever_` with ElvUI (AceDB) and Details (per character), two characters.
fn tree(root: &Path) -> std::path::PathBuf {
    let p = root.join("World of Warcraft/_forever_");
    write(&p.join(".flavor.info"), "Product Flavor!STRING:0\nwowf\n");
    write(
        &p.join("Interface/AddOns/ElvUI/ElvUI.toc"),
        "## Title: ElvUI\n## SavedVariables: ElvDB\n",
    );
    write(
        &p.join("Interface/AddOns/Details/Details.toc"),
        "## SavedVariablesPerCharacter: DetailsCharDB\n",
    );
    let acct = p.join("WTF/Account/SMOKE");
    write(
        &acct.join("SavedVariables/ElvUI.lua"),
        "ElvDB = {\n\t[\"profileKeys\"] = {\n\t\t[\"Aerin - Stormcall\"] = \"Aerin - Stormcall\",\n\t},\n\t[\"profiles\"] = {\n\t\t[\"Aerin - Stormcall\"] = { [\"scale\"] = 0.7 },\n\t},\n}\n",
    );
    write(&acct.join("SavedVariables/Ryna.lua"), "RynaDB = {}\n");
    write(
        &acct.join("Stormcall/Aerin/SavedVariables/Details.lua"),
        "DetailsCharDB = { [\"w\"] = 1 }\n",
    );
    write(
        &acct.join("Stormcall/Aerin/config-cache.wtf"),
        "SET a \"1\"\n",
    );
    std::fs::create_dir_all(acct.join("Doomhowl/Bryn")).unwrap();
    root.join("World of Warcraft")
}

fn ryna(state: &Path, game: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ryna"))
        .env("RYNA_STATE_DIR", state)
        .env("RYNA_ISSUER", "https://settings-test.invalid")
        .env("RYNA_API", "https://settings-test.invalid")
        .env_remove("RYNA_INSTALL")
        .arg("--install")
        .arg(game)
        .args(args)
        .output()
        .unwrap()
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn list_diff_and_copy_on_a_fake_tree() {
    let dir = tempfile::tempdir().unwrap();
    let game = tree(dir.path());
    let state = dir.path().join("state");
    let acct = game.join("_forever_/WTF/Account/SMOKE");

    let list = ok(&ryna(&state, &game, &["settings", "list"]));
    assert!(list.contains("SMOKE in"), "{list}");
    assert!(
        list.contains("Aerin-Stormcall") && list.contains("Bryn-Doomhowl"),
        "{list}"
    );
    assert!(list.contains("ElvUI") && list.contains("AceDB"), "{list}");
    assert!(
        list.contains("Details") && list.contains("per character"),
        "{list}"
    );
    assert!(list.contains("Aerin-Stormcall: config-cache.wtf"), "{list}");
    assert!(
        !list.contains("Ryna.lua") && !list.contains(" Ryna "),
        "{list}"
    );

    let diff = ok(&ryna(
        &state,
        &game,
        &[
            "settings",
            "diff",
            "--a",
            "Aerin-Stormcall",
            "--b",
            "Bryn-Doomhowl",
        ],
    ));
    assert!(
        diff.contains("ElvUI") && diff.contains("only Aerin-Stormcall"),
        "{diff}"
    );

    let dry = ok(&ryna(
        &state,
        &game,
        &[
            "settings",
            "copy",
            "--from",
            "Aerin-Stormcall",
            "--to",
            "Bryn-Doomhowl",
            "--dry-run",
        ],
    ));
    assert!(
        dry.contains("Would write Doomhowl/Bryn/SavedVariables/Details.lua"),
        "{dry}"
    );
    assert!(
        dry.contains("Would write SavedVariables/ElvUI.lua"),
        "{dry}"
    );
    assert!(dry.contains("Dry run: nothing was written."), "{dry}");
    assert!(!acct.join("Doomhowl/Bryn/SavedVariables").exists());

    // --force: the test can't know whether a game is open on the machine running it.
    let copied = ok(&ryna(
        &state,
        &game,
        &[
            "settings",
            "copy",
            "--from",
            "aerin-stormcall",
            "--to",
            "Bryn-Doomhowl",
            "--addon",
            "ElvUI",
            "--addon",
            "config-cache.wtf",
            "--force",
        ],
    ));
    assert!(
        copied.contains("2 file(s) written, 1 kept as .ryna.bak"),
        "{copied}"
    );
    assert!(acct.join("SavedVariables/ElvUI.lua.ryna.bak").is_file());
    assert_eq!(
        std::fs::read_to_string(acct.join("Doomhowl/Bryn/config-cache.wtf")).unwrap(),
        "SET a \"1\"\n"
    );
    let elv = std::fs::read_to_string(acct.join("SavedVariables/ElvUI.lua")).unwrap();
    assert!(
        elv.contains("[\"Bryn - Doomhowl\"] = \"Bryn - Doomhowl\""),
        "{elv}"
    );
    // Details wasn't asked for.
    assert!(!acct
        .join("Doomhowl/Bryn/SavedVariables/Details.lua")
        .exists());

    let diff = ok(&ryna(
        &state,
        &game,
        &[
            "settings",
            "diff",
            "--a",
            "Aerin-Stormcall",
            "--b",
            "Bryn-Doomhowl",
        ],
    ));
    assert!(diff.contains("ElvUI") && diff.contains("same"), "{diff}");

    let bad = ryna(
        &state,
        &game,
        &[
            "settings",
            "copy",
            "--from",
            "Aerin",
            "--to",
            "Bryn-Doomhowl",
        ],
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("Name-Realm"));
}
