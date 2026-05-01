/// Shared data structures between server and client.
/// These types use `dipa::DiffPatch` to support efficient delta encoding.
///
/// dipa limitations:
///   - No HashMap support — we use Vec for ordered collections
///   - max_fields_per_batch defaults to 4; structs with >4 fields need `#[dipa(max_fields_per_batch = N)]`
use dipa::DiffPatch;
use serde::{Deserialize, Serialize};

/// Global stats visible to every connected client (real-time, server-computed)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct GlobalStats {
    /// Number of WebSocket sessions currently open
    pub online_count: u32,
    /// Total registered users
    pub total_users: u32,
    /// Total todos across all users
    pub total_todos: u32,
}

/// Per-user public profile data
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct UserView {
    pub id: String,
    pub name: String,
    pub bio: String,
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

/// A single todo item owned by one user
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub completed: bool,
    pub owner_id: String,
}

/// Base session/global data sent to every client.
/// 5 fields — needs max_fields_per_batch = 5.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
#[dipa(max_fields_per_batch = 5)]
pub struct ClientStore {
    pub session_id: String,
    pub logged_in: bool,
    pub router: RouterState,
    /// Global stats that change whenever any session connects/disconnects or any user acts
    pub global: GlobalStats,
    /// Flash messages visible regardless of login state (e.g. signup errors)
    pub messages: Vec<Message>,
}

/// Per-user data sent only when logged in.
/// Kept separate to stay within dipa's 4-field batch limit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct UserStore {
    pub user: Option<UserView>,
    /// This user's own todos
    pub todos: Vec<TodoItem>,
}

/// Full client-visible snapshot = ClientStore + optional UserStore
/// A logged-in user currently online, visible to everyone
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct OnlineUser {
    pub id: String,
    pub name: String,
    pub bio: String,
}

/// A post on the public chat board
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct ChatPost {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub text: String,
}

/// Public data visible to all clients — chat board + online user list
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct PublicData {
    pub online_users: Vec<OnlineUser>,
    pub chat_posts: Vec<ChatPost>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, DiffPatch)]
pub struct FullStore {
    pub base: ClientStore,
    /// Public board + online users — sent to every connected client
    pub public_data: PublicData,
    pub user_data: Option<UserStore>,
}

/// Operations the client sends to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
    UserLogin { username: String, password: String },
    UserSignUp { username: String, password: String },
    UserLogOut,
    RouterChange { name: String },
    SessionRemoveMessage { id: String },
    // --- Todo ops ---
    AddTodo { text: String },
    ToggleTodo { id: String },
    DeleteTodo { id: String },
    // --- Profile ops ---
    UpdateBio { bio: String },
    // --- Public board ---
    PostChat { text: String },
    Ping,
}

/// Frames the server sends back to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Full state snapshot on first connect
    Snapshot(Box<FullStore>),
    /// Delta patch — postcard-encoded `<FullStore as Diffable>::DeltaOwned`
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

