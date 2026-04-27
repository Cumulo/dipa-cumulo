/// Shared data structures between server and client.
/// These types use `dipa::DiffPatch` to support efficient delta encoding.
///
/// dipa limitations:
///   - No HashMap support — we use Vec for messages
///   - max_fields_per_batch defaults to 4; structs with >4 fields need `#[dipa(max_fields_per_batch = N)]`
use dipa::DiffPatch;
use serde::{Deserialize, Serialize};

/// Per-user public data exposed to the client
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct UserView {
    pub id: String,
    pub name: String,
}

/// Per-session router state
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct RouterState {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct Message {
    pub id: String,
    pub text: String,
    pub kind: MessageKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, DiffPatch)]
pub enum MessageKind {
    Info,
    Error,
}

/// What each connected client sees — the "twig" projection.
///
/// Fields ≤ 4 so dipa's derive macro is happy with the default batch size.
/// Messages are a Vec (full replace) to avoid HashMap complexity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct ClientStore {
    pub session_id: String,
    pub logged_in: bool,
    pub member_count: u32,
    pub router: RouterState,
}

/// Additional per-user data, sent only when logged in.
/// Kept separate to stay within dipa's field limit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct UserStore {
    pub user: Option<UserView>,
    /// Error / info messages as a list; replaced wholesale on change.
    pub messages: Vec<Message>,
}

/// Full client-visible snapshot = ClientStore + optional UserStore
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct FullStore {
    pub base: ClientStore,
    pub user_data: Option<UserStore>,
}

/// Operations the client can send to the server (mirrors Calcit `Op` enum)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
    UserLogin { username: String, password: String },
    UserSignUp { username: String, password: String },
    UserLogOut,
    RouterChange { name: String },
    SessionRemoveMessage { id: String },
    Ping,
}

/// Frames the server sends back to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Full state snapshot on first connect
    Snapshot(FullStore),
    /// Delta patch — bincode-encoded `<FullStore as Diffable>::DeltaOwned`
    Patch(Vec<u8>),
    /// Pong reply
    Pong,
}

/// Frames the client sends to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Op(Op),
    Ping,
}

