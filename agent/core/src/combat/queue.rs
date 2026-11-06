//! Fights waiting to upload, on disk in `<state>/combat/queue/` so a quit (or a crash, or a day
//! offline) loses none: per fight a `<seq>-<key>.json` (the summary and how far its upload got),
//! its gzipped digest beside it (`.digest.gz`) and, when raw lines are kept, `.raw.gz`.
//!
//! Upload order is queue order: summaries in batches of up to 50 and 8 MiB
//! (`POST /v1/wow/agent/encounters`, answered with each fight's canonical `id`), then each
//! fight's digest (`PUT …/:id/digest?key=`) and raw lines (`PUT …/:id/raw?key=`). The calls retry
//! like snapshots; a fight is removed once everything it has is up. A `DIGESTS_OFF` refusal means
//! the API takes no digests: that fight's is skipped, and so are the rest of the session's.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::summary::Summary;
use super::CombatOptions;
use crate::fix::Fix;
use crate::push::{Api, FightBody, PushError, ENCOUNTERS_LIMIT, ENCOUNTERS_PER_CALL};

/// The API's answer for a key another person's agent already uploaded: done, not retried.
pub const KEY_TAKEN: &str = "KEY_TAKEN";

/// After this many refusals (not outages) a fight is set aside in `failed/`.
pub const MAX_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub seq: u64,
    pub key: String,
    pub queued_at: i64,
    pub players: usize,
    pub summary: Summary,
    /// The canonical encounter, once the summary is up.
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub has_digest: bool,
    #[serde(default)]
    pub digest_done: bool,
    #[serde(default)]
    pub has_raw: bool,
    #[serde(default)]
    pub raw_done: bool,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Item {
    fn stem(&self) -> String {
        format!("{:016}-{}", self.seq, self.key)
    }
}

/// What happened to one fight's upload.
#[derive(Debug, Clone, PartialEq)]
pub enum Uploaded {
    /// Everything it had is up.
    Done { item: Item },
    /// Another person's agent had already uploaded it (`KEY_TAKEN`): dropped, nothing to retry.
    Taken { item: Item },
    /// It was refused `MAX_ATTEMPTS` times and set aside.
    GaveUp { item: Item, message: String },
}

/// One upload round: what finished, and the error that stopped it, if one did.
#[derive(Debug, Default)]
pub struct Round {
    pub uploaded: Vec<Uploaded>,
    pub error: Option<(PushError, Option<Item>)>,
    /// The API said it takes no digests.
    pub digests_off: bool,
}

#[derive(Debug, Clone)]
pub struct Queue {
    dir: PathBuf,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn id_text(id: &Value) -> String {
    match id {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Queue {
    pub fn new(state_dir: &Path) -> Queue {
        Queue {
            dir: state_dir.join("combat").join("queue"),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every queued fight, in order. Unreadable entries are skipped (and left for a person to see).
    pub fn items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .filter_map(|p| std::fs::read(&p).ok())
                    .filter_map(|b| serde_json::from_slice::<Item>(&b).ok())
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by_key(|i| i.seq);
        items
    }

    pub fn len(&self) -> usize {
        self.items().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, key: &str) -> bool {
        self.items().iter().any(|i| i.key == key)
    }

    fn path(&self, item: &Item, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{ext}", item.stem()))
    }

    fn save(&self, item: &Item) -> std::io::Result<()> {
        let body = serde_json::to_vec(item).map_err(std::io::Error::other)?;
        write_atomic(&self.path(item, "json"), &body)
    }

    fn remove(&self, item: &Item) {
        for ext in ["digest.gz", "raw.gz", "json"] {
            let _ = std::fs::remove_file(self.path(item, ext));
        }
    }

    fn set_aside(&self, item: &Item) {
        let failed = self.dir.parent().unwrap_or(&self.dir).join("failed");
        if std::fs::create_dir_all(&failed).is_ok() {
            for ext in ["digest.gz", "raw.gz", "json"] {
                let from = self.path(item, ext);
                if from.exists() {
                    let _ = std::fs::rename(&from, failed.join(format!("{}.{ext}", item.stem())));
                }
            }
        }
        self.remove(item);
    }

    /// Queue one fight with its gzipped digest and raw lines (either may be absent). The bodies
    /// are written before the summary, so a crash between leaves no summary without them.
    pub fn push(
        &self,
        summary: Summary,
        players: usize,
        digest: Option<Vec<u8>>,
        raw: Option<Vec<u8>>,
        now: i64,
    ) -> std::io::Result<Item> {
        std::fs::create_dir_all(&self.dir)?;
        let seq = self.items().last().map_or(1, |i| i.seq + 1);
        let item = Item {
            seq,
            key: summary.key.clone(),
            queued_at: now,
            players,
            summary,
            id: None,
            merged: None,
            has_digest: digest.is_some(),
            digest_done: false,
            has_raw: raw.is_some(),
            raw_done: false,
            attempts: 0,
            last_error: None,
        };
        if let Some(d) = &digest {
            write_atomic(&self.path(&item, "digest.gz"), d)?;
        }
        if let Some(r) = &raw {
            write_atomic(&self.path(&item, "raw.gz"), r)?;
        }
        self.save(&item)?;
        Ok(item)
    }

    /// A refusal (not an outage or a lost sign-in) counts against the fight.
    fn refused(&self, item: &mut Item, e: &PushError, round: &mut Round) -> bool {
        item.attempts += 1;
        item.last_error = Some(e.to_string());
        if item.attempts >= MAX_ATTEMPTS {
            self.set_aside(item);
            round.uploaded.push(Uploaded::GaveUp {
                item: item.clone(),
                message: e.to_string(),
            });
            true
        } else {
            let _ = self.save(item);
            false
        }
    }

    /// Upload what's queued, in order, until it's all up or a call fails.
    pub fn upload(&self, api: &Api<'_>, opts: &CombatOptions, digests_off: bool) -> Round {
        let mut round = Round {
            digests_off,
            ..Round::default()
        };
        let mut items = self.items();

        // 1. The summaries not yet up, in batches.
        let mut batches: Vec<Vec<usize>> = Vec::new();
        let mut batch: Vec<usize> = Vec::new();
        let mut size = 32;
        for (i, item) in items.iter().enumerate() {
            if item.id.is_some() {
                continue;
            }
            let len = serde_json::to_vec(&item.summary).map_or(0, |b| b.len()) + 1;
            if !batch.is_empty()
                && (batch.len() >= ENCOUNTERS_PER_CALL || size + len > ENCOUNTERS_LIMIT)
            {
                batches.push(std::mem::take(&mut batch));
                size = 32;
            }
            batch.push(i);
            size += len;
        }
        if !batch.is_empty() {
            batches.push(batch);
        }
        let mut taken: Vec<usize> = Vec::new();
        while !batches.is_empty() {
            let batch = batches.remove(0);
            let body = json!({ "encounters": batch.iter().map(|&i| &items[i].summary).collect::<Vec<_>>() });
            match api.encounters(&body) {
                Ok(data) => {
                    let answers = data
                        .get("encounters")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    for &i in &batch {
                        let item = &mut items[i];
                        let answer = answers
                            .iter()
                            .find(|a| a.get("key").and_then(Value::as_str) == Some(&item.key));
                        let code = answer
                            .and_then(|a| a.pointer("/error/code"))
                            .and_then(Value::as_str);
                        if code == Some(KEY_TAKEN) {
                            taken.push(i);
                            continue;
                        }
                        match answer.and_then(|a| a.get("id")).filter(|id| !id.is_null()) {
                            Some(id) => {
                                item.id = Some(id.clone());
                                item.merged = answer
                                    .and_then(|a| a.get("merged"))
                                    .and_then(Value::as_bool);
                                item.last_error = None;
                                let _ = self.save(item);
                            }
                            None => {
                                let e = PushError::Refused {
                                    code: "NO_ID".into(),
                                    message: "the API's answer had no id for this fight".into(),
                                };
                                self.refused(item, &e, &mut round);
                            }
                        }
                    }
                }
                // Another person's agent uploaded this fight's key: it's done here. In a batch,
                // the fights go one at a time to find which.
                Err(PushError::Refused { code, .. }) if code == KEY_TAKEN => {
                    if batch.len() == 1 {
                        taken.push(batch[0]);
                    } else {
                        for (n, &i) in batch.iter().enumerate() {
                            batches.insert(n, vec![i]);
                        }
                    }
                }
                Err(e) => {
                    if matches!(e.fix(), Fix::SignIn | Fix::Network) {
                        round.error = Some((e, None));
                        return round;
                    }
                    for &i in &batch {
                        let mut item = items[i].clone();
                        self.refused(&mut item, &e, &mut round);
                        items[i] = item;
                    }
                    round.error = Some((e, None));
                    return round;
                }
            }
        }
        taken.sort_unstable();
        taken.dedup();
        for &i in taken.iter().rev() {
            let item = items.remove(i);
            self.remove(&item);
            round.uploaded.push(Uploaded::Taken { item });
        }

        // 2. Each fight's bodies, then it's done.
        for mut item in items {
            let Some(id) = item.id.clone().map(|id| id_text(&id)) else {
                continue;
            };
            if !self.path(&item, "json").exists() {
                continue; // set aside above
            }
            for which in [FightBody::Digest, FightBody::Raw] {
                let (has, done, wanted, ext) = match which {
                    FightBody::Digest => (
                        item.has_digest,
                        item.digest_done,
                        opts.digests && !round.digests_off,
                        "digest.gz",
                    ),
                    FightBody::Raw => (item.has_raw, item.raw_done, opts.raw, "raw.gz"),
                };
                if !has || done {
                    continue;
                }
                let mark = |item: &mut Item| match which {
                    FightBody::Digest => item.digest_done = true,
                    FightBody::Raw => item.raw_done = true,
                };
                let bytes = match std::fs::read(self.path(&item, ext)) {
                    Ok(b) if wanted => b,
                    // Turned off since it was queued, or the file is gone: nothing to send.
                    _ => {
                        mark(&mut item);
                        let _ = self.save(&item);
                        continue;
                    }
                };
                match api.fight_body(which, &id, &item.key, bytes) {
                    Ok(_) => {
                        mark(&mut item);
                        let _ = self.save(&item);
                    }
                    Err(PushError::Refused { code, .. }) if code == "DIGESTS_OFF" => {
                        round.digests_off = true;
                        mark(&mut item);
                        let _ = self.save(&item);
                    }
                    Err(PushError::TooLarge { .. }) => {
                        // Too big to send: the summary stands without it.
                        mark(&mut item);
                        let _ = self.save(&item);
                    }
                    Err(e) => {
                        if matches!(e.fix(), Fix::SignIn | Fix::Network) {
                            round.error = Some((e, Some(item)));
                            return round;
                        }
                        if !self.refused(&mut item, &e, &mut round) {
                            round.error = Some((e, Some(item.clone())));
                        }
                        return round;
                    }
                }
            }
            self.remove(&item);
            round.uploaded.push(Uploaded::Done { item });
        }
        round
    }
}
