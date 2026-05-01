/// Typed structs mirroring the benchmark fixture JSON schema.
/// Each struct derives cumulo_dipa::DiffPatch for binary structural diffing,
/// matching how dipa-cumulo uses dipa in its server/client sync pipeline.
///
/// Field count notes (dipa default max_fields_per_batch = 4):
///   User    7 fields → max_fields_per_batch = 7
///   Reply   7 fields → max_fields_per_batch = 7
///   Thread 10 fields → max_fields_per_batch = 10
use cumulo_dipa::DiffPatch;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // dipa-cumulo/
        .unwrap()
        .parent() // cumulo/
        .unwrap()
        .join("recollect.mbt/bench/fixtures")
        .join(name)
}

pub fn load_fixture<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = fixture_path(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
}

// ─── Leaf types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct Meta {
    pub version: i64,
    pub generated_at: i64,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
#[dipa(field_batching_strategy = "no_batching")]
pub struct User {
    pub id: String,
    pub name: String,
    pub avatar: String,
    pub online: bool,
    pub bio: String,
    pub role: String,
    pub joined_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct Reaction {
    pub emoji: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
#[dipa(field_batching_strategy = "no_batching")]
pub struct Reply {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub text: String,
    pub ts: i64,
    pub reactions: Vec<Reaction>,
    pub edited: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
#[dipa(field_batching_strategy = "no_batching")]
pub struct Thread {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub title: String,
    pub text: String,
    pub ts: i64,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub reactions: Vec<Reaction>,
    pub replies: Vec<Reply>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub threads: Vec<Thread>,
}

/// Root state — mirrors state_base.json / state_*.json
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct ChatState {
    pub meta: Meta,
    pub users: Vec<User>,
    pub channels: Vec<Channel>,
}
