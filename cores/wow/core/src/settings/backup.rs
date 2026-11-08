//! Backups to ryna.io (docs/wow.md section 15 in the API repository), opt-in, as the signed-in
//! person.
//!
//! A backup is one zip of the files the person chose (addon settings, and by name the game's own
//! per-character files), with a `manifest.json` listing each file's path, size and sha256. The
//! agent encrypts the whole zip with AES-256-GCM (a random 96-bit nonce, the backup's id as the
//! associated data, the 128-bit tag at the end) under a data key the API mints for that backup,
//! and uploads the ciphertext in parts. The API never sees a plaintext backup; it does hold the
//! key that wraps the data keys, which is what "stored encrypted for you on ryna.io" means.
//!
//! Paths are WTF-relative, with the account folder's name replaced by its key (the first 16 hex of
//! its sha256, as the snapshot's `account.key`): `Account/<key>/SavedVariables/ElvUI.lua`,
//! `Account/<key>/<Realm>/<Name>/config-cache.wtf`. The file list goes to the API in the clear,
//! and the account's name never leaves the computer. A restore maps the key back to the folder
//! here whose name hashes to it.
//!
//! A restore checks everything before it writes anything: the ciphertext's size and sha256, the
//! decryption (which fails on any changed byte), each file's sha256 against the manifest, and that
//! every path is one this mod writes (a SavedVariables `.lua` or one of the game's files, never
//! `Ryna.lua`). Then each file it replaces is kept as `<file>.ryna.bak` and the files are written,
//! with the game closed.

use std::collections::BTreeSet;
use std::io::{Cursor, Read as _, Write as _};
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::inventory::{Inventory, GAME_FILES};
use crate::discovery::{Install, SAVED_VARIABLES_FILE};
use crate::http::Method;
use crate::push::{path_segment, Api, SETTINGS_BACKUPS_PATH};
use crate::snapshot::account_key;

/// The API's cap on one backup's ciphertext.
pub const MAX_BYTES: usize = 50 * 1024 * 1024;
/// The API keeps this many per person (completing another deletes the oldest).
pub const KEEP: usize = 10;
/// Every part but the last is between these.
pub const PART_MIN: usize = 5 * 1024 * 1024;
pub const PART_MAX: usize = 10 * 1024 * 1024;
/// The API takes at most this many files in one backup.
pub const MAX_FILES: usize = 2000;
pub const MANIFEST_NAME: &str = "manifest.json";
pub const MANIFEST_SCHEMA: i64 = 1;

/// Which files go in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Include {
    /// Addon settings (the default).
    Addons,
    /// The game's own per-character files.
    Game,
    All,
}

impl Include {
    pub fn parse(s: &str) -> Result<Include> {
        match s.trim().to_ascii_lowercase().as_str() {
            "addons" => Ok(Include::Addons),
            "game" => Ok(Include::Game),
            "all" => Ok(Include::All),
            other => bail!("--include is addons, game or all, not {other:?}"),
        }
    }

    fn addons(self) -> bool {
        matches!(self, Include::Addons | Include::All)
    }

    fn game(self) -> bool {
        matches!(self, Include::Game | Include::All)
    }
}

/// One file in a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// WTF-relative, the account folder as its key: `Account/<key>/SavedVariables/ElvUI.lua`.
    pub path: String,
    pub bytes: u64,
    /// Lowercase hex.
    pub sha256: String,
}

/// `manifest.json`, inside the zip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema: i64,
    pub created_at: i64,
    pub machine: String,
    pub flavor: String,
    /// The account's key.
    pub account: String,
    pub files: Vec<ManifestFile>,
}

/// A backup built on this machine, before it's sent.
#[derive(Debug, Clone)]
pub struct Built {
    pub manifest: Manifest,
    pub zip: Vec<u8>,
}

/// How far a backup or restore has got, for a front end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// `building`, `uploading`, `completing`, `downloading`, `checking`, `writing`.
    pub stage: &'static str,
    pub done: usize,
    pub total: usize,
}

/// A backup on ryna.io (`GET /backups`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub id: String,
    pub created_at: Value,
    pub bytes: u64,
    #[serde(default)]
    pub machine: Option<String>,
    #[serde(default)]
    pub flavor: Option<String>,
    #[serde(default)]
    pub account: Option<String>,
    /// How many files.
    #[serde(default)]
    pub files: u64,
}

/// What [`backup`] did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDone {
    pub id: String,
    pub created_at: Value,
    /// Ciphertext bytes.
    pub bytes: u64,
    pub files: usize,
    pub parts: usize,
}

/// One file a restore writes (or would).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreFile {
    /// Account-relative (`SavedVariables/ElvUI.lua`).
    pub path: String,
    pub bytes: u64,
    /// A file is there now, and is kept as `<path>.ryna.bak`.
    pub replaces: bool,
}

/// What [`restore`] did, or in a dry run would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub id: String,
    /// The account folder it goes into.
    pub account: String,
    pub files: Vec<RestoreFile>,
    /// False for a dry run.
    pub written: bool,
    /// Account-relative paths of the `.ryna.bak` copies made.
    pub backups: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The account-relative paths of what `include` takes from `inv`.
pub fn chosen_files(inv: &Inventory, include: Include) -> Vec<String> {
    let mut out = BTreeSet::new();
    if include.addons() {
        for a in &inv.addons {
            out.extend(a.files.iter().map(|f| f.path.clone()));
        }
    }
    if include.game() {
        out.extend(inv.game_files.iter().map(|f| f.path.clone()));
    }
    out.into_iter().collect()
}

/// Build the zip: `manifest.json` first, then each chosen file under its backup path.
pub fn build(inv: &Inventory, include: Include, machine: &str, now: i64) -> Result<Built> {
    let chosen = chosen_files(inv, include);
    if chosen.is_empty() {
        bail!(
            "there's nothing to back up: no settings files were found for {}",
            inv.account
        );
    }
    if chosen.len() > MAX_FILES {
        bail!(
            "that's {} files, over the {MAX_FILES} a backup takes",
            chosen.len()
        );
    }
    let mut files = Vec::new();
    let mut contents = Vec::new();
    for rel in &chosen {
        let bytes = std::fs::read(inv.abs(rel)).with_context(|| format!("reading {rel}"))?;
        files.push(ManifestFile {
            path: format!("Account/{}/{rel}", inv.account_key),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
        });
        contents.push(bytes);
    }
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        created_at: now,
        machine: machine.to_string(),
        flavor: inv.flavor.clone().unwrap_or_else(|| "wowf".into()),
        account: inv.account_key.clone(),
        files,
    };
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    zip.start_file(MANIFEST_NAME, options)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    for (f, bytes) in manifest.files.iter().zip(&contents) {
        zip.start_file(f.path.as_str(), options)?;
        zip.write_all(bytes)?;
    }
    let zip = zip.finish()?.into_inner();
    Ok(Built { manifest, zip })
}

/// AES-256-GCM over `plain`: the ciphertext with its 16-byte tag at the end. The backup's id is the
/// associated data, so a ciphertext can't be passed off as another backup's.
pub fn encrypt(plain: &[u8], key: &[u8; 32], nonce: &[u8; 12], id: &str) -> Result<Vec<u8>> {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key))
        .encrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: plain,
                aad: id.as_bytes(),
            },
        )
        .map_err(|_| anyhow!("the backup couldn't be encrypted"))
}

pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12], id: &str) -> Result<Vec<u8>> {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key))
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: id.as_bytes(),
            },
        )
        .map_err(|_| {
            anyhow!("the backup couldn't be decrypted (it was changed, or isn't this backup's): nothing was written")
        })
}

/// A fresh random content nonce.
pub fn new_nonce() -> [u8; 12] {
    Aes256Gcm::generate_nonce(&mut OsRng).into()
}

/// `len` bytes cut into parts of `size` (the last one shorter, when it doesn't divide).
pub fn parts(len: usize, size: usize) -> Vec<Range<usize>> {
    let size = size.max(1);
    (0..len.div_ceil(size))
        .map(|i| i * size..((i + 1) * size).min(len))
        .collect()
}

/// The part size: the API's largest, kept within its own rule.
pub fn part_size(min: usize, max: usize) -> Result<usize> {
    if min == 0 || max < min {
        bail!("the API's part sizes don't make sense ({min} to {max})");
    }
    Ok(max)
}

fn b64_array<const N: usize>(v: &Value, what: &str) -> Result<[u8; N]> {
    let text = v
        .as_str()
        .ok_or_else(|| anyhow!("the API's answer has no {what}"))?;
    let bytes = STANDARD
        .decode(text)
        .map_err(|_| anyhow!("the API's {what} isn't base64"))?;
    <[u8; N]>::try_from(bytes).map_err(|_| anyhow!("the API's {what} isn't {N} bytes"))
}

fn backup_url(id: &str, rest: &str) -> String {
    format!("{SETTINGS_BACKUPS_PATH}/{}{rest}", path_segment(id))
}

/// Back `inv` up: build, ask the API for a backup and its data key, encrypt, upload the parts,
/// complete. `progress` hears each step.
pub fn backup(
    api: &Api,
    inv: &Inventory,
    include: Include,
    machine: &str,
    now: i64,
    progress: &mut dyn FnMut(Progress),
) -> Result<BackupDone> {
    progress(Progress {
        stage: "building",
        done: 0,
        total: 1,
    });
    let built = build(inv, include, machine, now)?;
    // The tag adds 16 bytes; refuse before a backup is made on the API at all.
    if built.zip.len() + 16 > MAX_BYTES {
        bail!(
            "the backup is {} MB, over the {} MB a backup can be: leave the game's files out, or back up fewer addons",
            (built.zip.len() + 16).div_ceil(1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        );
    }
    let nonce = new_nonce();
    let created = api
        .settings_call(
            Method::Post,
            SETTINGS_BACKUPS_PATH,
            Some(&json!({
                "machine": machine,
                "flavor": built.manifest.flavor,
                "account": built.manifest.account,
                "nonce": STANDARD.encode(nonce),
                "files": built.manifest.files,
            })),
            Duration::from_secs(60),
        )
        .context("starting the backup")?;
    let id = created
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("the API's answer has no backup id"))?
        .to_string();
    let key: [u8; 32] = b64_array(&created["dataKey"], "data key")?;
    let max_bytes = created
        .get("maxBytes")
        .and_then(Value::as_u64)
        .map_or(MAX_BYTES, |m| m as usize);
    let size = part_size(
        created
            .pointer("/partBytes/min")
            .and_then(Value::as_u64)
            .map_or(PART_MIN, |m| m as usize),
        created
            .pointer("/partBytes/max")
            .and_then(Value::as_u64)
            .map_or(PART_MAX, |m| m as usize),
    )?;
    let ciphertext = encrypt(&built.zip, &key, &nonce, &id)?;
    if ciphertext.len() > max_bytes {
        bail!(
            "the backup is {} bytes, over the {max_bytes} the API takes",
            ciphertext.len()
        );
    }
    let ranges = parts(ciphertext.len(), size);
    let mut uploaded = Vec::new();
    for (i, r) in ranges.iter().enumerate() {
        progress(Progress {
            stage: "uploading",
            done: i,
            total: ranges.len(),
        });
        let n = i + 1;
        let answer = api
            .settings_put(
                &backup_url(&id, &format!("/parts/{n}")),
                ciphertext[r.clone()].to_vec(),
                size,
                Duration::from_secs(300),
            )
            .with_context(|| format!("uploading part {n} of {}", ranges.len()))?;
        let etag = answer
            .get("etag")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("the API's answer for part {n} has no etag"))?;
        uploaded.push(json!({ "n": n, "etag": etag }));
    }
    progress(Progress {
        stage: "completing",
        done: ranges.len(),
        total: ranges.len(),
    });
    let done = api
        .settings_call(
            Method::Post,
            &backup_url(&id, "/complete"),
            Some(&json!({
                "parts": uploaded,
                "bytes": ciphertext.len(),
                "sha256": sha256_hex(&ciphertext),
            })),
            Duration::from_secs(120),
        )
        .context("finishing the backup")?;
    Ok(BackupDone {
        id,
        created_at: done.get("createdAt").cloned().unwrap_or(Value::Null),
        bytes: ciphertext.len() as u64,
        files: built.manifest.files.len(),
        parts: ranges.len(),
    })
}

/// The person's backups, newest first.
pub fn list(api: &Api) -> Result<Vec<BackupSummary>> {
    let data = api
        .settings_call(
            Method::Get,
            SETTINGS_BACKUPS_PATH,
            None,
            Duration::from_secs(30),
        )
        .context("listing the backups")?;
    let backups = data.get("backups").cloned().unwrap_or(Value::Array(vec![]));
    serde_json::from_value(backups).context("the API's list of backups isn't one")
}

/// Delete a backup (its object and its row).
pub fn delete(api: &Api, id: &str) -> Result<()> {
    api.settings_call(
        Method::Delete,
        &backup_url(id, ""),
        None,
        Duration::from_secs(30),
    )
    .with_context(|| format!("deleting backup {id}"))?;
    Ok(())
}

/// The account folder in `install` a backup of account `key` goes into: `chosen` when given, else
/// the one whose name hashes to `key`.
pub fn account_for(install: &Install, key: &str, chosen: Option<&str>) -> Result<String> {
    let all = super::inventory::accounts(install);
    if let Some(c) = chosen {
        return all
            .into_iter()
            .find(|a| a.eq_ignore_ascii_case(c))
            .ok_or_else(|| {
                anyhow!(
                    "there's no account folder {c} in {}",
                    install.path.display()
                )
            });
    }
    all.iter()
        .find(|a| account_key(a) == key)
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "this backup is of a game account that isn't in {} (its folders: {}); choose one to restore it into",
                install.path.display(),
                if all.is_empty() { "none".to_string() } else { all.join(", ") }
            )
        })
}

/// The account-relative path for a backup path of account `key`, if it's one this mod writes:
/// `SavedVariables/<x>.lua`, `<Realm>/<Name>/SavedVariables/<x>.lua` or `<Realm>/<Name>/<game file>`,
/// each part a plain name, and never `Ryna.lua`.
pub fn restorable_path(path: &str, key: &str) -> Option<String> {
    let rest = path
        .strip_prefix("Account/")?
        .strip_prefix(key)?
        .strip_prefix('/')?;
    let parts: Vec<&str> = rest.split('/').collect();
    let plain = |p: &str| {
        !p.is_empty()
            && p != "."
            && p != ".."
            && !p.contains(['\\', ':', '\0'])
            && !p.starts_with(' ')
            && !p.ends_with([' ', '.'])
    };
    if !parts.iter().all(|p| plain(p)) {
        return None;
    }
    let lua = |f: &str| {
        f.ends_with(".lua") && f.len() > 4 && !f.eq_ignore_ascii_case(SAVED_VARIABLES_FILE)
    };
    let ok = match parts.as_slice() {
        ["SavedVariables", f] => lua(f),
        [_, _, "SavedVariables", f] => lua(f),
        [_, _, f] => GAME_FILES.contains(f),
        _ => false,
    };
    ok.then(|| rest.to_string())
}

/// A backup's files: account-relative path and bytes.
pub type RestoredFiles = Vec<(String, Vec<u8>)>;

/// The files in a decrypted backup, checked against its manifest: each one there, its size and
/// sha256 as listed, its path one this mod writes. Any failure is an error and nothing is returned.
pub fn read_zip(zip_bytes: &[u8]) -> Result<(Manifest, RestoredFiles)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(zip_bytes)).context("the backup isn't a zip")?;
    let manifest: Manifest = {
        let entry = zip
            .by_name(MANIFEST_NAME)
            .context("the backup has no manifest.json")?;
        let mut text = Vec::new();
        entry.take(16 * 1024 * 1024).read_to_end(&mut text)?;
        serde_json::from_slice(&text).context("the backup's manifest.json isn't one")?
    };
    if manifest.schema != MANIFEST_SCHEMA {
        bail!(
            "the backup's manifest is schema {}, which this agent doesn't know: update Ryna",
            manifest.schema
        );
    }
    let mut out = Vec::new();
    for f in &manifest.files {
        let rel = restorable_path(&f.path, &manifest.account)
            .ok_or_else(|| anyhow!("the backup has a file this mod doesn't write: {}", f.path))?;
        let entry = zip
            .by_name(&f.path)
            .with_context(|| format!("the backup lists {} but doesn't hold it", f.path))?;
        let mut bytes = Vec::new();
        // Never more than the manifest says, whatever the zip claims.
        entry.take(f.bytes + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != f.bytes || sha256_hex(&bytes) != f.sha256 {
            bail!(
                "{} in the backup isn't what its manifest says: nothing was written",
                f.path
            );
        }
        out.push((rel, bytes));
    }
    Ok((manifest, out))
}

/// Restore backup `id` into `install`: download, check, decrypt, check every file, then (unless
/// `dry_run`) keep each file it replaces as `.ryna.bak` and write. `account` chooses the account
/// folder; else it's the one the backup was made from. Refuses to write while the game runs unless
/// `force`.
#[allow(clippy::too_many_arguments)]
pub fn restore(
    api: &Api,
    id: &str,
    install: &Install,
    account: Option<&str>,
    dry_run: bool,
    game_running: bool,
    force: bool,
    progress: &mut dyn FnMut(Progress),
) -> Result<RestoreReport> {
    if !dry_run {
        super::refuse_if_running(game_running, force)?;
    }
    progress(Progress {
        stage: "downloading",
        done: 0,
        total: 1,
    });
    let opened = api
        .settings_call(
            Method::Post,
            &backup_url(id, "/open"),
            Some(&json!({})),
            Duration::from_secs(30),
        )
        .with_context(|| format!("opening backup {id}"))?;
    let key: [u8; 32] = b64_array(&opened["dataKey"], "data key")?;
    let nonce: [u8; 12] = b64_array(&opened["nonce"], "nonce")?;
    let want_bytes = opened.get("bytes").and_then(Value::as_u64);
    let want_sha = opened
        .get("sha256")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase);
    let ciphertext = api
        .settings_download(&backup_url(id, "/content"), Duration::from_secs(600))
        .with_context(|| format!("downloading backup {id}"))?;
    progress(Progress {
        stage: "checking",
        done: 0,
        total: 1,
    });
    if want_bytes != Some(ciphertext.len() as u64)
        || want_sha.as_deref() != Some(sha256_hex(&ciphertext).as_str())
    {
        bail!("the downloaded backup doesn't match what ryna.io says it is (its size or sha256): nothing was written");
    }
    let plain = decrypt(&ciphertext, &key, &nonce, id)?;
    let (manifest, files) = read_zip(&plain)?;
    let folder = account_for(install, &manifest.account, account)?;
    let root: PathBuf = install.path.join("WTF").join("Account").join(&folder);
    let abs = |rel: &str| rel.split('/').fold(root.clone(), |p, c| p.join(c));
    let mut report = RestoreReport {
        id: id.to_string(),
        account: folder.clone(),
        files: files
            .iter()
            .map(|(rel, bytes)| RestoreFile {
                path: rel.clone(),
                bytes: bytes.len() as u64,
                replaces: abs(rel).is_file(),
            })
            .collect(),
        written: false,
        backups: Vec::new(),
    };
    if dry_run {
        return Ok(report);
    }
    // Asked again: the download can take a while, and the game may have been started.
    super::refuse_if_running(game_running, force)?;
    for (i, (rel, bytes)) in files.iter().enumerate() {
        progress(Progress {
            stage: "writing",
            done: i,
            total: files.len(),
        });
        if super::replace_file(&abs(rel), bytes)?.is_some() {
            report.backups.push(format!("{rel}{}", super::BAK_SUFFIX));
        }
    }
    report.written = true;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::auth::Auth;
    use crate::config::test_config;
    use crate::http::{Request, Response, Transport};
    use crate::identity::testing::test_key;
    use crate::identity::{verify, KEY_HEADER, SIGNATURE_HEADER};
    use crate::settings::inventory::inventory;
    use crate::settings::testing::{account_dir, elvui_tree};
    use crate::util::testing::FakeClock;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    const NOW: i64 = 1_800_000_000;

    #[derive(Default)]
    struct Stored {
        ready: bool,
        nonce: String,
        account: String,
        files: usize,
        parts: BTreeMap<u64, Vec<u8>>,
        content: Vec<u8>,
        sha256: String,
        machine: String,
    }

    /// The API of docs/wow.md section 15, in memory: checks the bearer and the device key's
    /// signature on every POST, PUT and DELETE, keeps parts, and serves the ciphertext back.
    struct FakeApi {
        key: [u8; 32],
        part_min: usize,
        part_max: usize,
        backups: RefCell<BTreeMap<String, Stored>>,
        next: Cell<u32>,
        /// Requests seen: method, path, body.
        seen: RefCell<Vec<(Method, String, Vec<u8>)>>,
        /// Flip a byte of the content served back.
        tamper: Cell<bool>,
    }

    impl FakeApi {
        fn new(part_min: usize, part_max: usize) -> Self {
            FakeApi {
                key: [7u8; 32],
                part_min,
                part_max,
                backups: RefCell::new(BTreeMap::new()),
                next: Cell::new(1),
                seen: RefCell::new(Vec::new()),
                tamper: Cell::new(false),
            }
        }

        fn ok(data: Value) -> Response {
            Response {
                status: 200,
                content_type: "application/json".into(),
                body: json!({ "ok": true, "data": data }).to_string().into_bytes(),
            }
        }

        fn refused(code: &str) -> Response {
            Response {
                status: 200,
                content_type: "application/json".into(),
                body: json!({ "ok": false, "error": { "code": code, "message": code } })
                    .to_string()
                    .into_bytes(),
            }
        }

        fn not_found() -> Response {
            Response {
                status: 404,
                content_type: "application/json".into(),
                body: json!({ "ok": false, "error": { "code": "NOT_FOUND", "message": "no" } })
                    .to_string()
                    .into_bytes(),
            }
        }

        fn methods(&self) -> Vec<(Method, String)> {
            self.seen
                .borrow()
                .iter()
                .map(|(m, p, _)| (*m, p.clone()))
                .collect()
        }
    }

    impl Transport for FakeApi {
        fn post(&self, req: &Request) -> Result<Response, String> {
            let path = req
                .url
                .strip_prefix("https://api.test")
                .expect("the test API")
                .to_string();
            self.seen
                .borrow_mut()
                .push((req.method, path.clone(), req.body.clone()));
            assert_eq!(req.bearer.as_deref(), Some("tok"), "{path}: as the person");
            let header = |name: &str| {
                req.headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.clone())
            };
            if req.method != Method::Get {
                let (Some(k), Some(sig)) = (header(KEY_HEADER), header(SIGNATURE_HEADER)) else {
                    panic!("{path}: unsigned {:?}", req.method);
                };
                assert!(verify(&k, &req.body, &sig), "{path}: signed over its body");
            }
            let rest = path
                .strip_prefix(SETTINGS_BACKUPS_PATH)
                .expect("a backups route");
            let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
            let mut backups = self.backups.borrow_mut();
            Ok(match (req.method, segs.as_slice()) {
                (Method::Post, []) => {
                    let body: Value = serde_json::from_slice(&req.body).unwrap();
                    let id = format!("00000000-0000-4000-8000-{:012}", self.next.get());
                    self.next.set(self.next.get() + 1);
                    backups.insert(
                        id.clone(),
                        Stored {
                            nonce: body["nonce"].as_str().unwrap().into(),
                            account: body["account"].as_str().unwrap().into(),
                            files: body["files"].as_array().unwrap().len(),
                            machine: body["machine"].as_str().unwrap().into(),
                            ..Stored::default()
                        },
                    );
                    Self::ok(json!({
                        "id": id,
                        "dataKey": STANDARD.encode(self.key),
                        "partBytes": { "min": self.part_min, "max": self.part_max },
                        "maxBytes": MAX_BYTES,
                        "expiresAt": NOW + 86_400,
                    }))
                }
                (Method::Put, [id, "parts", n]) => {
                    let Some(b) = backups.get_mut(*id).filter(|b| !b.ready) else {
                        return Ok(Self::not_found());
                    };
                    if req.content_type != "application/octet-stream"
                        || req.body.len() > self.part_max
                    {
                        return Ok(Self::refused("PART_SIZE"));
                    }
                    let n: u64 = n.parse().unwrap();
                    b.parts.insert(n, req.body.clone());
                    Self::ok(json!({ "n": n, "etag": format!("etag-{n}") }))
                }
                (Method::Post, [id, "complete"]) => {
                    let Some(b) = backups.get_mut(*id) else {
                        return Ok(Self::not_found());
                    };
                    let body: Value = serde_json::from_slice(&req.body).unwrap();
                    let listed: Vec<u64> = body["parts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            let n = p["n"].as_u64().unwrap();
                            assert_eq!(p["etag"], json!(format!("etag-{n}")));
                            n
                        })
                        .collect();
                    assert_eq!(listed, b.parts.keys().copied().collect::<Vec<_>>());
                    // Every part but the last within the rule.
                    let sizes: Vec<usize> = b.parts.values().map(Vec::len).collect();
                    for s in &sizes[..sizes.len() - 1] {
                        assert!(*s >= self.part_min && *s <= self.part_max, "{sizes:?}");
                    }
                    b.content = b.parts.values().flatten().copied().collect();
                    if body["bytes"].as_u64() != Some(b.content.len() as u64) {
                        return Ok(Self::refused("BACKUP_MISMATCH"));
                    }
                    b.sha256 = body["sha256"].as_str().unwrap().into();
                    assert_eq!(b.sha256, sha256_hex(&b.content));
                    b.ready = true;
                    Self::ok(json!({ "id": id, "createdAt": NOW, "bytes": b.content.len() }))
                }
                (Method::Get, []) => Self::ok(json!({
                    "backups": backups.iter().filter(|(_, b)| b.ready).rev().map(|(id, b)| json!({
                        "id": id, "createdAt": NOW, "bytes": b.content.len(), "machine": b.machine,
                        "flavor": "wowf", "account": b.account, "files": b.files,
                    })).collect::<Vec<_>>()
                })),
                (Method::Post, [id, "open"]) => match backups.get(*id) {
                    Some(b) if b.ready => Self::ok(json!({
                        "dataKey": STANDARD.encode(self.key),
                        "nonce": b.nonce,
                        "bytes": b.content.len(),
                        "sha256": b.sha256,
                    })),
                    Some(_) => Self::refused("BACKUP_PENDING"),
                    None => Self::not_found(),
                },
                (Method::Get, [id, "content"]) => match backups.get(*id) {
                    Some(b) if b.ready => {
                        let mut body = b.content.clone();
                        if self.tamper.get() {
                            body[0] ^= 1;
                        }
                        Response {
                            status: 200,
                            content_type: "application/octet-stream".into(),
                            body,
                        }
                    }
                    _ => Self::not_found(),
                },
                (Method::Delete, [id]) => match backups.remove(*id) {
                    Some(_) => Self::ok(json!({ "deleted": true })),
                    None => Self::not_found(),
                },
                other => panic!("no route {other:?}"),
            })
        }
    }

    fn with_api<T>(fake: &FakeApi, f: impl FnOnce(&Api) -> T) -> T {
        let config = test_config();
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, fake, &store, &clock);
        let key = test_key();
        let api = Api::new(&config.api, fake, &auth, Some(&key), &clock);
        f(&api)
    }

    fn quiet() -> impl FnMut(Progress) {
        |_| {}
    }

    #[test]
    fn parts_split_at_the_boundaries() {
        assert_eq!(parts(0, 10), Vec::<Range<usize>>::new());
        assert_eq!(parts(1, 10), vec![Range { start: 0, end: 1 }]);
        assert_eq!(parts(10, 10), vec![Range { start: 0, end: 10 }]);
        assert_eq!(parts(11, 10), [0..10, 10..11]);
        assert_eq!(parts(20, 10), [0..10, 10..20]);
        assert_eq!(parts(21, 10), [0..10, 10..20, 20..21]);
        let big = parts(MAX_BYTES, PART_MAX);
        assert_eq!(big.len(), 5);
        assert!(big.iter().all(|r| r.len() == PART_MAX));
        assert_eq!(part_size(PART_MIN, PART_MAX).unwrap(), PART_MAX);
        assert!(part_size(0, 1).is_err() && part_size(5, 4).is_err());
    }

    #[test]
    fn encryption_is_bound_to_the_key_the_nonce_and_the_id() {
        let key = [1u8; 32];
        let nonce = [2u8; 12];
        let ct = encrypt(b"settings", &key, &nonce, "id-1").unwrap();
        assert_eq!(ct.len(), b"settings".len() + 16);
        assert_eq!(decrypt(&ct, &key, &nonce, "id-1").unwrap(), b"settings");
        assert!(decrypt(&ct, &key, &nonce, "id-2").is_err());
        assert!(decrypt(&ct, &[9u8; 32], &nonce, "id-1").is_err());
        let mut bad = ct.clone();
        bad[3] ^= 1;
        assert!(decrypt(&bad, &key, &nonce, "id-1").is_err());
        assert_ne!(new_nonce(), new_nonce());
    }

    #[test]
    fn only_paths_this_mod_writes_are_restorable() {
        let k = "0123456789abcdef";
        let ok = |p: &str| restorable_path(&format!("Account/{k}/{p}"), k);
        assert_eq!(
            ok("SavedVariables/ElvUI.lua").as_deref(),
            Some("SavedVariables/ElvUI.lua")
        );
        assert!(ok("Stormcall/Aerin/SavedVariables/Details.lua").is_some());
        assert!(ok("Stormcall/Aerin/config-cache.wtf").is_some());
        assert!(ok("Argent Dawn/Aerin/AddOns.txt").is_some());
        for bad in [
            "SavedVariables/Ryna.lua",
            "Stormcall/Aerin/SavedVariables/Ryna.lua",
            "SavedVariables/../../../Interface/AddOns/x.lua",
            "../x/SavedVariables/a.lua",
            "SavedVariables/a.lua.bak",
            "Stormcall/Aerin/evil.exe",
            "SavedVariables/.lua",
            "a\\b/Aerin/config-cache.wtf",
            "C:/Aerin/config-cache.wtf",
            "",
        ] {
            assert_eq!(ok(bad), None, "{bad}");
        }
        assert_eq!(
            restorable_path("Account/other/SavedVariables/a.lua", k),
            None
        );
    }

    #[test]
    fn the_zip_holds_a_manifest_and_hashed_paths_and_no_account_name() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let built = build(&inv, Include::Addons, "test-host", NOW).unwrap();
        let paths: Vec<&str> = built
            .manifest
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        let k = account_key("ACCOUNT1");
        assert!(paths.contains(&format!("Account/{k}/SavedVariables/ElvUI.lua").as_str()));
        assert!(paths
            .contains(&format!("Account/{k}/Stormcall/Aerin/SavedVariables/Details.lua").as_str()));
        assert!(paths
            .iter()
            .all(|p| !p.contains("ACCOUNT1") && !p.ends_with("Ryna.lua")));
        assert!(
            paths.iter().all(|p| !p.ends_with(".wtf")),
            "game files only when asked"
        );
        let (manifest, files) = read_zip(&built.zip).unwrap();
        assert_eq!(manifest, built.manifest);
        assert_eq!(files.len(), paths.len());
        let all = build(&inv, Include::All, "test-host", NOW).unwrap();
        assert!(all.manifest.files.len() > built.manifest.files.len());
        let game = build(&inv, Include::Game, "test-host", NOW).unwrap();
        assert!(game
            .manifest
            .files
            .iter()
            .all(|f| !f.path.ends_with(".lua")));
        assert!(Include::parse("ADDONS").is_ok() && Include::parse("bogus").is_err());
    }

    #[test]
    fn a_manifest_that_disagrees_with_its_files_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let mut built = build(&inv, Include::Addons, "h", NOW).unwrap();
        built.manifest.files[0].sha256 = "0".repeat(64);
        // A zip whose manifest lies about a file.
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file(MANIFEST_NAME, opts).unwrap();
        zip.write_all(&serde_json::to_vec(&built.manifest).unwrap())
            .unwrap();
        let (_, files) = read_zip(&build(&inv, Include::Addons, "h", NOW).unwrap().zip).unwrap();
        for (f, (_, bytes)) in built.manifest.files.iter().zip(&files) {
            zip.start_file(f.path.as_str(), opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        let lying = zip.finish().unwrap().into_inner();
        let e = read_zip(&lying).unwrap_err().to_string();
        assert!(e.contains("isn't what its manifest says"), "{e}");
    }

    #[test]
    fn back_up_in_parts_then_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let root = account_dir(&i);
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        // Small parts, so even a small backup goes up in several.
        let fake = FakeApi::new(64, 256);
        let mut stages = Vec::new();
        let done = with_api(&fake, |api| {
            backup(api, &inv, Include::All, "test-host", NOW, &mut |p| {
                stages.push(p.stage)
            })
        })
        .unwrap();
        assert!(done.parts > 1, "{done:?}");
        assert_eq!(done.parts as u64, done.bytes.div_ceil(256));
        assert_eq!(done.files, chosen_files(&inv, Include::All).len());
        assert_eq!(stages.first(), Some(&"building"));
        assert_eq!(stages.last(), Some(&"completing"));
        let seen = fake.methods();
        assert_eq!(seen[0], (Method::Post, SETTINGS_BACKUPS_PATH.to_string()));
        assert_eq!(
            seen.iter().filter(|(m, _)| *m == Method::Put).count(),
            done.parts
        );
        assert_eq!(
            seen.last().unwrap().1,
            format!("{SETTINGS_BACKUPS_PATH}/{}/complete", done.id)
        );
        // What reached the API: no plaintext and no account name.
        for (_, _, body) in fake.seen.borrow().iter() {
            let text = String::from_utf8_lossy(body);
            assert!(!text.contains("ACCOUNT1") && !text.contains("fontSize"));
        }
        let create: Value = serde_json::from_slice(&fake.seen.borrow()[0].2).unwrap();
        assert_eq!(create["account"], json!(account_key("ACCOUNT1")));
        assert_eq!(create["machine"], json!("test-host"));
        assert_eq!(
            STANDARD
                .decode(create["nonce"].as_str().unwrap())
                .unwrap()
                .len(),
            12
        );

        let listed = with_api(&fake, list).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, done.id);
        assert_eq!(listed[0].files as usize, done.files);

        // Change things locally, then a dry run says what it'd put back and writes nothing.
        let elv = root.join("SavedVariables/ElvUI.lua");
        let original = std::fs::read(&elv).unwrap();
        std::fs::write(&elv, "ElvDB = { [\"broken\"] = true }\n").unwrap();
        std::fs::remove_file(root.join("Stormcall/Aerin/macros-cache.txt")).unwrap();
        let dry = with_api(&fake, |api| {
            restore(api, &done.id, &i, None, true, true, false, &mut quiet())
        })
        .unwrap();
        assert!(!dry.written && dry.backups.is_empty());
        assert_eq!(dry.account, "ACCOUNT1");
        let elv_row = dry
            .files
            .iter()
            .find(|f| f.path == "SavedVariables/ElvUI.lua")
            .unwrap();
        assert!(elv_row.replaces);
        let macros = dry
            .files
            .iter()
            .find(|f| f.path == "Stormcall/Aerin/macros-cache.txt")
            .unwrap();
        assert!(!macros.replaces);
        assert!(!root.join("Stormcall/Aerin/macros-cache.txt").exists());

        // The game running: refused, nothing written.
        let e = with_api(&fake, |api| {
            restore(api, &done.id, &i, None, false, true, false, &mut quiet())
        })
        .unwrap_err();
        assert!(e.to_string().contains("close it first"));
        assert_ne!(std::fs::read(&elv).unwrap(), original);

        let report = with_api(&fake, |api| {
            restore(api, &done.id, &i, None, false, false, false, &mut quiet())
        })
        .unwrap();
        assert!(report.written);
        assert_eq!(std::fs::read(&elv).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(root.join("SavedVariables/ElvUI.lua.ryna.bak")).unwrap(),
            "ElvDB = { [\"broken\"] = true }\n"
        );
        assert!(root.join("Stormcall/Aerin/macros-cache.txt").is_file());
        assert!(report
            .backups
            .contains(&"SavedVariables/ElvUI.lua.ryna.bak".to_string()));
        // Ryna.lua was never in it, so it's as it was.
        assert_eq!(
            std::fs::read_to_string(root.join("SavedVariables/Ryna.lua")).unwrap(),
            "RynaDB = {}\n"
        );

        with_api(&fake, |api| delete(api, &done.id)).unwrap();
        assert!(with_api(&fake, list).unwrap().is_empty());
        assert!(with_api(&fake, |api| delete(api, &done.id)).is_err());
    }

    #[test]
    fn a_sha256_mismatch_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let root = account_dir(&i);
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let fake = FakeApi::new(PART_MIN, PART_MAX);
        let done = with_api(&fake, |api| {
            backup(api, &inv, Include::Addons, "h", NOW, &mut quiet())
        })
        .unwrap();
        assert_eq!(done.parts, 1);
        let elv = root.join("SavedVariables/ElvUI.lua");
        std::fs::write(&elv, "changed").unwrap();
        fake.tamper.set(true);
        let e = with_api(&fake, |api| {
            restore(api, &done.id, &i, None, false, false, false, &mut quiet())
        })
        .unwrap_err()
        .to_string();
        assert!(e.contains("nothing was written"), "{e}");
        assert_eq!(std::fs::read_to_string(&elv).unwrap(), "changed");
        assert!(!root.join("SavedVariables/ElvUI.lua.ryna.bak").exists());
    }

    #[test]
    fn a_restore_finds_its_account_or_is_told_one() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        std::fs::create_dir_all(i.path.join("WTF/Account/OTHER")).unwrap();
        assert_eq!(
            account_for(&i, &account_key("ACCOUNT1"), None).unwrap(),
            "ACCOUNT1"
        );
        assert_eq!(
            account_for(&i, &account_key("ACCOUNT1"), Some("other")).unwrap(),
            "OTHER"
        );
        let e = account_for(&i, &account_key("GONE"), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("ACCOUNT1, OTHER"), "{e}");
        assert!(account_for(&i, "x", Some("NOPE")).is_err());
    }

    #[test]
    fn nothing_to_back_up_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let i = crate::settings::testing::install(dir.path(), &[]);
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        assert!(build(&inv, Include::All, "h", NOW)
            .unwrap_err()
            .to_string()
            .contains("nothing to back up"));
    }
}
