use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use dipa::Diffable;
use futures_util::{SinkExt, StreamExt};
use shared::{
    ChatPost, ClientMsg, ClientStore, FullStore, GlobalStats, Message as AppMessage, MessageKind,
    OnlineUser, Op, PublicData, RouterState, ServerMsg, TodoItem, UserStore, UserView,
};
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Domain types (server-internal, never sent as-is)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct User {
    id: String,
    name: String,
    password_md5: String,
    bio: String,
}

#[derive(Debug, Clone)]
struct Session {
    id: String,
    user_id: Option<String>,
    router: RouterState,
    messages: Vec<AppMessage>,
}

impl Session {
    fn new(id: String) -> Self {
        Self {
            id,
            user_id: None,
            router: RouterState { name: "home".into() },
            messages: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared app state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AppDb {
    users: HashMap<String, User>,
    sessions: HashMap<String, Session>,
    /// All todos across all users, in insertion order
    todos: Vec<TodoItem>,
    /// Public chat board (capped at 200 posts)
    chat_posts: Vec<ChatPost>,
}

type SharedDb = Arc<Mutex<AppDb>>;
/// Broadcast channel to ask the render loop to push updates.
type Notify = broadcast::Sender<()>;

#[derive(Clone)]
struct AppState {
    db: SharedDb,
    notify: Notify,
    /// per-session last-sent store snapshot (for diffing)
    caches: Arc<DashMap<String, FullStore>>,
}

// ---------------------------------------------------------------------------
// Twig projection: compute what a session should see
// ---------------------------------------------------------------------------

fn twig_container(db: &AppDb, session: &Session) -> FullStore {
    let logged_in = session.user_id.is_some();
    let global = GlobalStats {
        online_count: db.sessions.len() as u32,
        total_users: db.users.len() as u32,
        total_todos: db.todos.len() as u32,
    };

    // Online users list — all sessions that have a logged-in user, sorted by name
    let mut online_users: Vec<OnlineUser> = db
        .sessions
        .values()
        .filter_map(|s| s.user_id.as_ref().and_then(|uid| db.users.get(uid)))
        .map(|u| OnlineUser { id: u.id.clone(), name: u.name.clone(), bio: u.bio.clone() })
        .collect();
    online_users.sort_by(|a, b| a.name.cmp(&b.name));

    let public_data = PublicData {
        online_users,
        chat_posts: db.chat_posts.clone(),
    };
    let user_data = if logged_in {
        let user = session.user_id.as_ref().and_then(|uid| db.users.get(uid)).map(|u| UserView {
            id: u.id.clone(),
            name: u.name.clone(),
            bio: u.bio.clone(),
        });
        let todos = session
            .user_id
            .as_ref()
            .map(|uid| db.todos.iter().filter(|t| &t.owner_id == uid).cloned().collect())
            .unwrap_or_default();
        Some(UserStore {
            user,
            todos,
        })
    } else {
        None
    };
    FullStore {
        base: ClientStore {
            logged_in,
            session_id: session.id.clone(),
            router: session.router.clone(),
            global,
            messages: session.messages.clone(),
        },
        public_data,
        user_data,
    }
}

// ---------------------------------------------------------------------------
// Updater: pure-style functions that return a new DB
// ---------------------------------------------------------------------------

fn updater(mut db: AppDb, op: Op, sid: &str, op_id: &str) -> AppDb {
    match op {
        Op::UserSignUp { username, password } => {
            let already_exists = db.users.values().any(|u| u.name == username);
            if already_exists {
                if let Some(session) = db.sessions.get_mut(sid) {
                    let msg_id = Uuid::new_v4().to_string();
                    session.messages.push(AppMessage { id: msg_id, text: "Username already taken".into(), kind: MessageKind::Error });
                }
            } else {
                let password_md5 = format!("{:x}", md5::compute(password));
                db.users.insert(
                    op_id.to_string(),
                    User { id: op_id.to_string(), name: username, password_md5, bio: String::new() },
                );
                if let Some(session) = db.sessions.get_mut(sid) {
                    session.user_id = Some(op_id.to_string());
                }
            }
        }
        Op::UserLogin { username, password } => {
            let password_md5 = format!("{:x}", md5::compute(password));
            let maybe_user = db.users.values().find(|u| u.name == username && u.password_md5 == password_md5).cloned();
            if let Some(user) = maybe_user {
                if let Some(session) = db.sessions.get_mut(sid) {
                    session.user_id = Some(user.id);
                }
            } else if let Some(session) = db.sessions.get_mut(sid) {
                let msg_id = Uuid::new_v4().to_string();
                session.messages.push(AppMessage { id: msg_id, text: "Invalid credentials".into(), kind: MessageKind::Error });
            }
        }
        Op::UserLogOut => {
            if let Some(session) = db.sessions.get_mut(sid) {
                session.user_id = None;
            }
        }
        Op::RouterChange { name } => {
            if let Some(session) = db.sessions.get_mut(sid) {
                session.router.name = name;
            }
        }
        Op::SessionRemoveMessage { id } => {
            if let Some(session) = db.sessions.get_mut(sid) {
                session.messages.retain(|m| m.id != id);
            }
        }
        Op::AddTodo { text } => {
            // Clone user_id first to avoid overlapping borrows
            let user_id = db.sessions.get(sid).and_then(|s| s.user_id.clone());
            if let Some(uid) = user_id {
                db.todos.push(TodoItem {
                    id: op_id.to_string(),
                    text,
                    completed: false,
                    owner_id: uid,
                });
            }
        }
        Op::ToggleTodo { id } => {
            let user_id = db.sessions.get(sid).and_then(|s| s.user_id.clone());
            if let Some(uid) = user_id {
                if let Some(todo) = db.todos.iter_mut().find(|t| t.id == id && t.owner_id == uid) {
                    todo.completed = !todo.completed;
                }
            }
        }
        Op::DeleteTodo { id } => {
            let user_id = db.sessions.get(sid).and_then(|s| s.user_id.clone());
            if let Some(uid) = user_id {
                db.todos.retain(|t| !(t.id == id && t.owner_id == uid));
            }
        }
        Op::UpdateBio { bio } => {
            let user_id = db.sessions.get(sid).and_then(|s| s.user_id.clone());
            if let Some(uid) = user_id {
                if let Some(user) = db.users.get_mut(&uid) {
                    user.bio = bio;
                }
            }
        }
        Op::PostChat { text } => {
            let author = db
                .sessions
                .get(sid)
                .and_then(|s| s.user_id.as_ref())
                .and_then(|uid| db.users.get(uid))
                .cloned();
            if let Some(author) = author {
                db.chat_posts.push(ChatPost {
                    id: op_id.to_string(),
                    author_id: author.id,
                    author_name: author.name,
                    text,
                });
                // Cap at 200 posts
                if db.chat_posts.len() > 200 {
                    db.chat_posts.drain(0..db.chat_posts.len() - 200);
                }
            }
        }
        Op::Ping => {} // handled at ws layer
    }
    db
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let sid = Uuid::new_v4().to_string();
    tracing::info!("New client connected: {sid}");

    // Register session
    {
        let mut db = state.db.lock().await;
        db.sessions.insert(sid.clone(), Session::new(sid.clone()));
    }
    let _ = state.notify.send(());

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send initial snapshot
    {
        let db = state.db.lock().await;
        let session = db.sessions.get(&sid).unwrap();
        let snapshot = twig_container(&db, session);
        state.caches.insert(sid.clone(), snapshot.clone());
        let encoded = bincode::serialize(&ServerMsg::Snapshot(Box::new(snapshot))).unwrap();
        let _ = ws_tx.send(Message::Binary(encoded.into())).await;
    }

    // Subscribe to broadcast
    let mut rx = state.notify.subscribe();
    let sid_clone = sid.clone();
    let state_clone = state.clone();

    // Spawn task: push diffs when notified
    let mut send_task = tokio::spawn(async move {
        while rx.recv().await.is_ok() {
            let db = state_clone.db.lock().await;
            let Some(session) = db.sessions.get(&sid_clone) else { break };
            let new_store = twig_container(&db, session);
            drop(db);

            let old_store = state_clone.caches.get(&sid_clone).map(|v| v.clone()).unwrap_or_default();

            let delta = old_store.create_delta_towards(&new_store);
            tracing::debug!("diff sid={} did_change={} old_logged_in={} new_logged_in={}", &sid_clone, delta.did_change, old_store.base.logged_in, new_store.base.logged_in);
            if delta.did_change {
                let patch_bytes = bincode::serialize(&delta.delta).unwrap();
                let msg = bincode::serialize(&ServerMsg::Patch(patch_bytes)).unwrap();
                if ws_tx.send(Message::Binary(msg.into())).await.is_err() {
                    break;
                }
                state_clone.caches.insert(sid_clone.clone(), new_store);
            }
        }
    });

    // Receive task: handle incoming ops
    let sid_recv = sid.clone();
    let state_recv = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    let Ok(client_msg) = bincode::deserialize::<ClientMsg>(&data) else { continue };
                    match client_msg {
                        ClientMsg::Ping => {
                            // pong is handled by send path via broadcast, nothing needed
                        }
                        ClientMsg::Op(op) => {
                            let op_id = Uuid::new_v4().to_string();
                            {
                                let mut guard = state_recv.db.lock().await;
                                // swap out db, run pure updater, swap back
                                let db = std::mem::take(&mut *guard);
                                *guard = updater(db, op, &sid_recv, &op_id);
                            }
                            let _ = state_recv.notify.send(());
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish, then clean up
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Disconnect
    let mut db = state.db.lock().await;
    db.sessions.remove(&sid);
    state.caches.remove(&sid);
    let _ = state.notify.send(());
    tracing::info!("Client disconnected: {sid}");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (notify, _) = broadcast::channel(64);
    let app_state = AppState {
        db: Arc::new(Mutex::new(AppDb::default())),
        notify,
        caches: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new("client/dist"))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = "0.0.0.0:5021";
    tracing::info!("Server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
