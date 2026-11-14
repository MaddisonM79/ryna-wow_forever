//! A fetched core's package (docs/ryna-agent-cores-mods.md 13.3): `ryna-core-<id>-<version>-<target>.tar.gz`
//! and its `.sig` on the same GitHub release as the app, signed in the release workflow with the
//! updater's key.
//!
//! - The URL is built here from the app's own version and target; no list of URLs is read from
//!   the network. `RYNA_CORE_PACKAGES` points elsewhere (a local folder, or an https URL, or http
//!   to localhost) for development and tests.
//! - The signature is checked against the updater's public key (`agent/app/updater.pub`) the way
//!   `tauri-plugin-updater` checks an app update: the key and the `.sig` are each the base64 of
//!   minisign's text, verified with `minisign-verify` (legacy signatures allowed, as the updater
//!   allows them). On top of that, a trusted comment that names a `file:` or a `version:` must
//!   name this package's, so a different signed package can't be swapped in. In a debug build
//!   only, `RYNA_CORE_PUBKEY` replaces the key.
//! - Nothing is unpacked before the signature is good. It unpacks into a temporary folder beside
//!   the target and is renamed to `<state>/cores/<id>/<version>/` only when whole; an entry that
//!   would land outside the folder (an absolute path, `..`, a link, a symlink that resolves out of
//!   it, anything but files, folders and symlinks) refuses the whole package.
//! - Other versions are removed only by [`prune_other_versions`], which the agent calls once the
//!   new one has completed a pass.

use std::path::{Component, Path, PathBuf};

use base64::Engine as _;

use crate::core::{CoreManifest, Runtime};

/// Where the app's releases are.
pub const RELEASES: &str = "https://github.com/MaddisonM79/ryna_agent/releases/download";
/// A folder, or a base URL, to take packages from instead of the release.
pub const PACKAGES_ENV: &str = "RYNA_CORE_PACKAGES";
/// Debug builds only: the public key (base64 of minisign's text) to check packages with.
pub const PUBKEY_ENV: &str = "RYNA_CORE_PUBKEY";
/// No package is larger than this (Node's binary and a bundle are well under it).
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;
/// The manifest at a package's root.
pub const MANIFEST_FILE: &str = "core.json";

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("there's no {0} package for this computer ({1})")]
    NoTarget(String, &'static str),
    #[error("couldn't download {0}")]
    Download(String),
    #[error("the package's signature doesn't check out ({0}); nothing was installed")]
    Signature(String),
    #[error("the package holds {0}, which would land outside its folder; nothing was installed")]
    Escapes(String),
    #[error("the package isn't a readable archive ({0}); nothing was installed")]
    Archive(String),
    #[error("the package's core.json {0}")]
    Manifest(String),
    #[error("{0} isn't an https:// URL or a folder")]
    Source(String),
    #[error("couldn't write the package: {0}")]
    Io(#[from] std::io::Error),
}

/// This build's target as the release names packages, or None where no package is published.
pub fn target() -> Option<&'static str> {
    target_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// The target triple for `os` and `arch` (`std::env::consts` values).
pub fn target_for(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        _ => return None,
    })
}

/// `ryna-core-<id>-<version>-<target>.tar.gz`.
pub fn file_name(id: &str, version: &str, target: &str) -> String {
    format!("ryna-core-{id}-{version}-{target}.tar.gz")
}

/// Where packages come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A base URL the file name is appended to.
    Url(String),
    /// A folder holding the files.
    Folder(PathBuf),
}

impl Source {
    /// The release for `version` (`v<version>`), unless `RYNA_CORE_PACKAGES` says otherwise.
    pub fn for_version(version: &str) -> Result<Source, PackageError> {
        match std::env::var(PACKAGES_ENV)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            Some(v) => Source::parse(&v),
            None => Ok(Source::Url(format!("{RELEASES}/v{version}"))),
        }
    }

    /// An https URL, http to a loopback host, or a folder.
    pub fn parse(v: &str) -> Result<Source, PackageError> {
        if v.contains("://") {
            let url = reqwest::Url::parse(v).map_err(|_| PackageError::Source(v.into()))?;
            let ok = match url.scheme() {
                "https" => url.host_str().is_some(),
                "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
                _ => false,
            };
            if !ok {
                return Err(PackageError::Source(v.into()));
            }
            Ok(Source::Url(v.trim_end_matches('/').to_string()))
        } else {
            Ok(Source::Folder(PathBuf::from(v)))
        }
    }

    fn get(
        &self,
        file: &str,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>, PackageError> {
        match self {
            Source::Url(base) => {
                crate::http::download(&format!("{base}/{file}"), MAX_BYTES, progress)
                    .map_err(PackageError::Download)
            }
            Source::Folder(dir) => {
                let bytes = std::fs::read(dir.join(file))
                    .map_err(|e| PackageError::Download(format!("{file}: {e}")))?;
                progress(bytes.len() as u64, Some(bytes.len() as u64));
                Ok(bytes)
            }
        }
    }
}

/// The key packages are checked with: `builtin` (the updater's, `agent/app/updater.pub`), or in
/// a debug build `RYNA_CORE_PUBKEY` when it is set.
pub fn public_key(builtin: &str) -> String {
    #[cfg(debug_assertions)]
    if let Some(k) = std::env::var(PUBKEY_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return k;
    }
    builtin.trim().to_string()
}

fn b64_text(what: &str, v: &str) -> Result<String, PackageError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(v.trim())
        .map_err(|e| PackageError::Signature(format!("the {what} isn't base64: {e}")))?;
    String::from_utf8(bytes).map_err(|_| PackageError::Signature(format!("the {what} isn't text")))
}

/// Check `bytes` against `sig` (the `.sig` file's contents) with `pubkey` (base64 of minisign's
/// public key text), as the updater does; then hold its trusted comment to `file` and `version`.
pub fn verify(
    bytes: &[u8],
    sig: &str,
    pubkey: &str,
    file: &str,
    version: &str,
) -> Result<(), PackageError> {
    let key = minisign_verify::PublicKey::decode(&b64_text("public key", pubkey)?)
        .map_err(|e| PackageError::Signature(format!("the public key: {e}")))?;
    let signature = minisign_verify::Signature::decode(&b64_text("signature", sig)?)
        .map_err(|e| PackageError::Signature(e.to_string()))?;
    key.verify(bytes, &signature, true)
        .map_err(|e| PackageError::Signature(e.to_string()))?;
    // Only now is the trusted comment trusted (the global signature covers it).
    for pair in signature.trusted_comment().split('\t') {
        match pair.split_once(':') {
            Some(("file", f)) if f.trim() != file => {
                return Err(PackageError::Signature(format!(
                    "it was signed for {}",
                    f.trim()
                )))
            }
            Some(("version", v)) if v.trim().trim_start_matches('v') != version => {
                return Err(PackageError::Signature(format!(
                    "it was signed for version {}",
                    v.trim()
                )))
            }
            _ => {}
        }
    }
    Ok(())
}

/// `<state>/cores/<id>`: the core's state folder (the sidecar's `stateDir`), its packages beside
/// what it keeps.
pub fn core_dir(state_dir: &Path, id: &str) -> PathBuf {
    state_dir.join("cores").join(id)
}

/// The unpacked package for `version`, when it's there whole (its `core.json` reads and agrees).
pub fn installed(state_dir: &Path, id: &str, version: &str) -> Option<(PathBuf, CoreManifest)> {
    let dir = core_dir(state_dir, id).join(version);
    let m = read_manifest(&dir, id, version).ok()?;
    Some((dir, m))
}

/// Read and check the package's `core.json`: this id and version, a sidecar whose `bin` is in it.
pub fn read_manifest(dir: &Path, id: &str, version: &str) -> Result<CoreManifest, PackageError> {
    let text = std::fs::read_to_string(dir.join(MANIFEST_FILE))
        .map_err(|e| PackageError::Manifest(format!("couldn't be read: {e}")))?;
    let m = CoreManifest::parse(&text).map_err(|e| PackageError::Manifest(e.to_string()))?;
    if m.id != id {
        return Err(PackageError::Manifest(format!("is for {}, not {id}", m.id)));
    }
    if m.version != version {
        return Err(PackageError::Manifest(format!(
            "is version {}, not {version}",
            m.version
        )));
    }
    let Runtime::Sidecar { bin, .. } = &m.runtime else {
        return Err(PackageError::Manifest("isn't a sidecar's".into()));
    };
    if !safe_relative(Path::new(bin)) || !dir.join(bin).is_file() {
        return Err(PackageError::Manifest(format!(
            "names {bin}, which isn't in it"
        )));
    }
    Ok(m)
}

/// Fetch, check and unpack the package for `id` at `version` from `source`, unless it's there.
pub fn fetch(
    state_dir: &Path,
    id: &str,
    version: &str,
    source: &Source,
    pubkey: &str,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(PathBuf, CoreManifest), PackageError> {
    if let Some(done) = installed(state_dir, id, version) {
        return Ok(done);
    }
    let target = target().ok_or(PackageError::NoTarget(id.into(), std::env::consts::OS))?;
    let file = file_name(id, version, target);
    let sig = source.get(&format!("{file}.sig"), &mut |_, _| {})?;
    let sig = String::from_utf8(sig)
        .map_err(|_| PackageError::Signature("the .sig isn't text".into()))?;
    let bytes = source.get(&file, progress)?;
    verify(&bytes, &sig, pubkey, &file, version)?;
    let dest = core_dir(state_dir, id).join(version);
    unpack(&bytes, &dest)?;
    let m = read_manifest(&dest, id, version).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&dest);
    })?;
    Ok((dest, m))
}

/// A path with only normal parts (no root, prefix, `.` or `..`) and at least one.
fn safe_relative(p: &Path) -> bool {
    p.components().next().is_some() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Unpack a `.tar.gz` (already checked) to `dest`, through a temporary folder beside it and a
/// rename; any entry that would land outside refuses it all, and nothing is left behind.
pub fn unpack(bytes: &[u8], dest: &Path) -> Result<(), PackageError> {
    let parent = dest
        .parent()
        .ok_or_else(|| PackageError::Archive("no folder to unpack into".into()))?;
    std::fs::create_dir_all(parent)?;
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = parent.join(format!(
        ".fetch-{name}-{}-{}",
        std::process::id(),
        crate::util::now_secs()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = unpack_into(bytes, &tmp).and_then(|()| {
        if dest.exists() {
            std::fs::remove_dir_all(dest)?;
        }
        std::fs::rename(&tmp, dest)?;
        Ok(())
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    result
}

fn unpack_into(bytes: &[u8], root: &Path) -> Result<(), PackageError> {
    let archive = |e: std::io::Error| PackageError::Archive(e.to_string());
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    tar.set_overwrite(true);
    tar.set_preserve_ownerships(false);
    for entry in tar.entries().map_err(archive)? {
        let mut entry = entry.map_err(archive)?;
        let raw = entry.path_bytes().into_owned();
        let shown = String::from_utf8_lossy(&raw).into_owned();
        let path = entry.path().map_err(archive)?.into_owned();
        // `./x` is how many tools write a root entry; anything else must be plain.
        let path: PathBuf = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        if path.as_os_str().is_empty() {
            continue;
        }
        if !safe_relative(&path) {
            return Err(PackageError::Escapes(shown));
        }
        match entry.header().entry_type() {
            tar::EntryType::Regular | tar::EntryType::Directory => {}
            tar::EntryType::Symlink => {
                let target = entry
                    .link_name()
                    .map_err(archive)?
                    .ok_or_else(|| PackageError::Escapes(shown.clone()))?;
                if target.is_absolute() || target.has_root() {
                    return Err(PackageError::Escapes(format!(
                        "{shown} -> {}",
                        target.display()
                    )));
                }
            }
            other => {
                return Err(PackageError::Escapes(format!("{shown} ({other:?})")));
            }
        }
        // tar's own check refuses writing through a symlink out of the folder too.
        if !entry.unpack_in(root).map_err(archive)? {
            return Err(PackageError::Escapes(shown));
        }
    }
    check_links(root)
}

/// Every symlink under `root` resolves to something under it (a chain of links included).
fn check_links(root: &Path) -> Result<(), PackageError> {
    let real_root = std::fs::canonicalize(root)?;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let path = e.path();
            let kind = e.file_type()?;
            if kind.is_symlink() {
                let shown = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                match std::fs::canonicalize(&path) {
                    Ok(real) if real.starts_with(&real_root) => {}
                    _ => return Err(PackageError::Escapes(shown)),
                }
            } else if kind.is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(())
}

/// Remove every other version of `id`'s package (and any fetch a crash left), keeping `keep` and
/// everything the core keeps for itself there. Call it once `keep` has completed a pass.
pub fn prune_other_versions(
    state_dir: &Path,
    id: &str,
    keep: &str,
) -> std::io::Result<Vec<PathBuf>> {
    let dir = core_dir(state_dir, id);
    let mut removed = Vec::new();
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Ok(removed);
    };
    for e in read {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        let a_version = is_version(&name);
        let leftover = name.starts_with(".fetch-");
        if (a_version && name != keep || leftover) && e.file_type()?.is_dir() {
            std::fs::remove_dir_all(e.path())?;
            removed.push(e.path());
        }
    }
    Ok(removed)
}

/// `x.y.z` (with an optional `-pre`): a package's folder, not one the core keeps.
fn is_version(name: &str) -> bool {
    let core = name.split('-').next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// A throwaway minisign key and signatures in the updater's encoding, for tests (this crate's,
/// and with feature `testing` the app's).
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub mod testing {
    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};

    pub struct TestKey {
        key: SigningKey,
        id: [u8; 8],
    }

    fn b64(v: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(v)
    }

    impl TestKey {
        pub fn new() -> Self {
            let mut seed = [0u8; 32];
            let mut id = [0u8; 8];
            getrandom::fill(&mut seed).unwrap();
            getrandom::fill(&mut id).unwrap();
            TestKey {
                key: SigningKey::from_bytes(&seed),
                id,
            }
        }

        /// What `updater.pub` holds: the base64 of minisign's public key text.
        pub fn public(&self) -> String {
            let mut bin = b"Ed".to_vec();
            bin.extend_from_slice(&self.id);
            bin.extend_from_slice(self.key.verifying_key().as_bytes());
            b64(format!("untrusted comment: minisign public key\n{}\n", b64(&bin)).as_bytes())
        }

        /// What a `.sig` holds: the base64 of minisign's signature text (a legacy, unhashed
        /// signature; the updater's check allows those as it allows the prehashed ones).
        pub fn sign(&self, data: &[u8], trusted: &str) -> String {
            let sig = self.key.sign(data).to_bytes();
            let mut bin = b"Ed".to_vec();
            bin.extend_from_slice(&self.id);
            bin.extend_from_slice(&sig);
            let mut global = sig.to_vec();
            global.extend_from_slice(trusted.as_bytes());
            let global = self.key.sign(&global).to_bytes();
            b64(format!(
                "untrusted comment: signature from tests\n{}\ntrusted comment: {trusted}\n{}\n",
                b64(&bin),
                b64(&global)
            )
            .as_bytes())
        }
    }

    impl Default for TestKey {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TestKey;
    use super::*;
    use std::io::Write;

    const V: &str = "0.5.0";

    fn manifest(id: &str, version: &str) -> String {
        serde_json::json!({ "id": id, "name": "Palworld", "version": version,
            "runtime": { "kind": "sidecar", "bin": "node", "args": ["agent.mjs", "serve"] },
            "platforms": ["macos"], "resource": "https://api.ryna.io/agent", "scopes": ["palworld:ingest"] })
        .to_string()
    }

    /// A gzipped tar of (path, contents) files, written by hand where a path is hostile (tar's
    /// own builder refuses those).
    fn tarball(files: &[(&str, &[u8])], links: &[(&str, &str)]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let raw_header = |path: &str, kind: tar::EntryType, size: u64| {
            let mut h = tar::Header::new_old();
            let name = &mut h.as_old_mut().name;
            name[..path.len()].copy_from_slice(path.as_bytes());
            h.set_entry_type(kind);
            h.set_size(size);
            h.set_mode(0o755);
            h
        };
        for (path, data) in files {
            let mut h = raw_header(path, tar::EntryType::Regular, data.len() as u64);
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        for (path, target) in links {
            let mut h = raw_header(path, tar::EntryType::Symlink, 0);
            let l = &mut h.as_old_mut().linkname;
            l[..target.len()].copy_from_slice(target.as_bytes());
            h.set_cksum();
            b.append(&h, std::io::empty()).unwrap();
        }
        let tar = b.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    fn good() -> Vec<u8> {
        tarball(
            &[
                ("core.json", manifest("palworld", V).as_bytes()),
                ("node", b"#!/bin/sh\n"),
                ("agent.mjs", b"// bundle"),
            ],
            // A link that stays inside is fine (Windows needs a privilege to make one).
            if cfg!(unix) {
                &[("lib/node-link", "../node")]
            } else {
                &[]
            },
        )
    }

    /// A folder source holding the package and its signature, as the release would.
    fn published(bytes: &[u8], key: &TestKey, trusted: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let file = file_name("palworld", V, target().unwrap());
        std::fs::write(dir.path().join(&file), bytes).unwrap();
        std::fs::write(
            dir.path().join(format!("{file}.sig")),
            key.sign(bytes, trusted),
        )
        .unwrap();
        dir
    }

    fn trusted() -> String {
        format!(
            "timestamp:1762900000\tfile:{}\tversion:{V}",
            file_name("palworld", V, target().unwrap())
        )
    }

    #[test]
    fn urls_and_targets_are_built_from_the_apps_version() {
        assert_eq!(target_for("macos", "aarch64"), Some("aarch64-apple-darwin"));
        assert_eq!(target_for("macos", "x86_64"), Some("x86_64-apple-darwin"));
        assert_eq!(target_for("plan9", "mips"), None);
        assert_eq!(
            file_name("palworld", "0.5.0", "aarch64-apple-darwin"),
            "ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            Source::parse("https://example.test/pkgs/").unwrap(),
            Source::Url("https://example.test/pkgs".into())
        );
        assert!(Source::parse("http://127.0.0.1:9000").is_ok());
        assert!(Source::parse("http://example.test").is_err());
        assert!(Source::parse("ftp://example.test").is_err());
        assert_eq!(
            Source::parse("/tmp/pkgs").unwrap(),
            Source::Folder("/tmp/pkgs".into())
        );
        if std::env::var(PACKAGES_ENV).is_err() {
            assert_eq!(
                Source::for_version("0.5.0").unwrap(),
                Source::Url(
                    "https://github.com/MaddisonM79/ryna_agent/releases/download/v0.5.0".into()
                )
            );
        }
    }

    #[test]
    fn a_good_signature_unpacks_through_a_rename() {
        let key = TestKey::new();
        let from = published(&good(), &key, &trusted());
        let state = tempfile::tempdir().unwrap();
        let mut seen = Vec::new();
        let (dir, m) = fetch(
            state.path(),
            "palworld",
            V,
            &Source::Folder(from.path().to_path_buf()),
            &key.public(),
            &mut |done, total| seen.push((done, total)),
        )
        .unwrap();
        assert_eq!(dir, state.path().join("cores/palworld/0.5.0"));
        assert_eq!(m.platforms.as_deref(), Some(&["macos".to_string()][..]));
        assert!(dir.join("node").is_file() && dir.join("agent.mjs").is_file());
        assert!(!seen.is_empty());
        // No temporary folder is left, and a second fetch finds it there.
        let left: Vec<_> = std::fs::read_dir(state.path().join("cores/palworld"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["0.5.0"]);
        assert!(installed(state.path(), "palworld", V).is_some());
        std::fs::remove_dir_all(from.path()).unwrap();
        assert!(fetch(
            state.path(),
            "palworld",
            V,
            &Source::Folder(from.path().to_path_buf()),
            &key.public(),
            &mut |_, _| {}
        )
        .is_ok());
    }

    fn refused(bytes: &[u8], key: &TestKey, signer: &TestKey, trusted: &str) -> PackageError {
        let from = published(bytes, signer, trusted);
        let state = tempfile::tempdir().unwrap();
        let e = fetch(
            state.path(),
            "palworld",
            V,
            &Source::Folder(from.path().to_path_buf()),
            &key.public(),
            &mut |_, _| {},
        )
        .unwrap_err();
        // Nothing at all under the core's folder: not the package, not a temporary folder.
        let core = state.path().join("cores/palworld");
        let left = std::fs::read_dir(&core).map(|d| d.count()).unwrap_or(0);
        assert_eq!(left, 0, "{e}: something was left in {}", core.display());
        e
    }

    #[test]
    fn a_bad_signature_unpacks_nothing() {
        let key = TestKey::new();
        // Signed by another key.
        let e = refused(&good(), &key, &TestKey::new(), &trusted());
        assert!(matches!(e, PackageError::Signature(_)), "{e}");
        // Signed by ours, then changed.
        let from = published(&good(), &key, &trusted());
        let file = file_name("palworld", V, target().unwrap());
        let mut bytes = std::fs::read(from.path().join(&file)).unwrap();
        let n = bytes.len();
        bytes[n / 2] ^= 1;
        std::fs::write(from.path().join(&file), &bytes).unwrap();
        let state = tempfile::tempdir().unwrap();
        let e = fetch(
            state.path(),
            "palworld",
            V,
            &Source::Folder(from.path().to_path_buf()),
            &key.public(),
            &mut |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, PackageError::Signature(_)), "{e}");
        assert!(!state.path().join("cores").exists());
        // Signed by ours, for another file or version.
        let e = refused(
            &good(),
            &key,
            &key,
            "timestamp:1\tfile:ryna-core-other-0.5.0-x.tar.gz",
        );
        assert!(e.to_string().contains("signed for"), "{e}");
        let e = refused(&good(), &key, &key, "timestamp:1\tversion:0.4.9");
        assert!(e.to_string().contains("0.4.9"), "{e}");
        // A .sig that isn't one.
        assert!(matches!(
            verify(b"x", "not base64!", &key.public(), "f", V),
            Err(PackageError::Signature(_))
        ));
    }

    #[test]
    fn an_entry_that_escapes_refuses_the_whole_package() {
        let key = TestKey::new();
        let json = manifest("palworld", V);
        let base: [(&str, &[u8]); 2] = [("core.json", json.as_bytes()), ("node", b"x")];
        for (files, links, what) in [
            (vec![("../evil", &b"x"[..])], vec![], "../evil"),
            (vec![("/tmp/evil", &b"x"[..])], vec![], "/tmp/evil"),
            (vec![("a/../../evil", &b"x"[..])], vec![], "evil"),
            (vec![], vec![("abs", "/etc/hosts")], "abs"),
            (vec![], vec![("out", "../../outside")], "out"),
            // Each link inside on its own, out together: `m` is `.`, so `l` is the parent.
            (vec![], vec![("m", "."), ("l", "m/..")], "l"),
        ] {
            // Making a symlink needs a privilege on Windows; the absolute one is refused unmade.
            if !cfg!(unix) && links.iter().any(|(_, t)| !t.starts_with('/')) {
                continue;
            }
            let mut all: Vec<(&str, &[u8])> = base.to_vec();
            all.extend(files);
            let e = refused(&tarball(&all, &links), &key, &key, "timestamp:1");
            assert!(matches!(e, PackageError::Escapes(_)), "{what}: {e}");
            assert!(e.to_string().contains(what), "{what}: {e}");
        }
    }

    #[test]
    fn a_package_whose_manifest_disagrees_is_removed() {
        let key = TestKey::new();
        let wrong = manifest("palworld", "0.4.0");
        let bytes = tarball(&[("core.json", wrong.as_bytes()), ("node", b"x")], &[]);
        let e = refused(&bytes, &key, &key, "timestamp:1");
        assert!(e.to_string().contains("0.4.0"), "{e}");
        let json = manifest("palworld", V);
        let no_bin = tarball(&[("core.json", json.as_bytes())], &[]);
        let e = refused(&no_bin, &key, &key, "timestamp:1");
        assert!(e.to_string().contains("node"), "{e}");
    }

    #[test]
    fn other_versions_go_only_when_asked_and_the_cores_own_files_stay() {
        let state = tempfile::tempdir().unwrap();
        let core = core_dir(state.path(), "palworld");
        for d in ["0.4.0", "0.5.0", ".fetch-0.5.0-1-2", "worlds"] {
            std::fs::create_dir_all(core.join(d)).unwrap();
        }
        std::fs::write(core.join("state.json"), "{}").unwrap();
        let mut removed = prune_other_versions(state.path(), "palworld", "0.5.0").unwrap();
        removed.sort();
        assert_eq!(removed, [core.join(".fetch-0.5.0-1-2"), core.join("0.4.0")]);
        assert!(core.join("0.5.0").is_dir() && core.join("worlds").is_dir());
        assert!(core.join("state.json").is_file());
        assert!(prune_other_versions(state.path(), "nothing", "1.0.0")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_signature_made_by_the_tauri_signer_checks_out() {
        // `tauri signer generate` and `tauri signer sign` (prehashed, as the release workflow
        // signs), with a throwaway key: the format checked is the release's own.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tauri-signed");
        let file = "ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz";
        let bytes = std::fs::read(dir.join(file)).unwrap();
        let sig = std::fs::read_to_string(dir.join(format!("{file}.sig"))).unwrap();
        let key = std::fs::read_to_string(dir.join("key.pub")).unwrap();
        verify(&bytes, &sig, &key, file, "0.5.0").unwrap();
        assert!(verify(b"changed\n", &sig, &key, file, "0.5.0").is_err());
        let e = verify(
            &bytes,
            &sig,
            &key,
            "ryna-core-palworld-0.5.0-x86_64-apple-darwin.tar.gz",
            "0.5.0",
        )
        .unwrap_err();
        assert!(e.to_string().contains("signed for"), "{e}");
    }

    #[test]
    fn the_key_is_the_builtin_unless_a_debug_build_overrides_it() {
        if std::env::var(PUBKEY_ENV).is_err() {
            assert_eq!(public_key(" abc \n"), "abc");
        }
        // The app's key decodes as a minisign public key.
        let shipped = include_str!("../../app/updater.pub");
        assert!(minisign_verify::PublicKey::decode(&b64_text("key", shipped).unwrap()).is_ok());
    }
}
