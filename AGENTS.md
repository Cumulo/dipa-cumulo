# dipa-calcium — Developer Agents & Debug Commands

## Project Stack

- **Server**: Rust + Axum 0.7 + Tokio, WebSocket on port 5021
- **Client**: Rust + wasm-pack → WASM, respo UI framework
- **Protocol**: bincode-serialized `ServerMsg` / `ClientMsg` over WebSocket
- **Diff**: dipa v0.1.1 binary diffs (`create_delta_towards`)
- **Known dipa bug**: `String` fields in nested structs return `did_change=false` → workaround via `ActionOp::RouteChange(String)` in client store

---

## Build Commands

```bash
# Build WASM client
cd client && wasm-pack build --target web --out-dir dist/pkg

# Build server (debug)
cargo build -p server

# Build all
cargo build
```

---

## Run / Restart Server

```bash
# Kill any running server and restart with logs
kill $(pgrep -f './target/debug/server') 2>/dev/null
RUST_LOG=info ./target/debug/server &

# Or via cargo run
RUST_LOG=info cargo run -p server
```

---

## Chrome Debug Browser

```bash
# Launch Chrome with remote debugging enabled
open -a "Google Chrome" --args \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/chrome-debug-calcium \
  --no-first-run \
  "http://localhost:5021/"
```

---

## chrome-devtools CLI

The `chrome-devtools` command connects to the debug Chrome instance (port 9222 by default).

```bash
# Check current pages
chrome-devtools list_pages

# Navigate (with cache bypass)
chrome-devtools navigate_page --url http://localhost:5021/ --ignoreCache

# Snapshot a11y tree (find element UIDs)
chrome-devtools take_snapshot

# Screenshot
chrome-devtools take_screenshot

# Click element by uid
chrome-devtools click <uid>

# Fill input
chrome-devtools fill <uid> "text value"

# Type text into focused element
chrome-devtools type_text "hello"

# Press key
chrome-devtools press_key "Enter"

# Evaluate JavaScript
chrome-devtools evaluate_script "() => document.title"

# List console messages (all types)
chrome-devtools list_console_messages

# List only errors
chrome-devtools list_console_messages --types error

# Get specific console message
chrome-devtools get_console_message <msgid>

# List network requests
chrome-devtools list_network_requests

# Filter network requests for errors
chrome-devtools list_network_requests 2>&1 | grep -i "404\|error\|fail"

# Wait for text to appear
chrome-devtools wait_for "some text" --timeout 5000
```

---

## WebSocket Quick Test (Python)

```bash
python3 - <<'EOF'
import websocket, json, bincode  # bincode not standard — use raw bytes
ws = websocket.create_connection("ws://localhost:5021/ws")
ws.close()
EOF
```

---

## Common Debug Flows

### Stale WASM cache error (`Invalid size`)

1. `chrome-devtools navigate_page --url http://localhost:5021/ --ignoreCache`
2. Or open DevTools → Application → Storage → Clear site data

### Full reset (server + cache)

```bash
kill $(pgrep -f './target/debug/server') 2>/dev/null
cd client && wasm-pack build --target web --out-dir dist/pkg && cd ..
RUST_LOG=info ./target/debug/server &
chrome-devtools navigate_page --url http://localhost:5021/ --ignoreCache
```

### Check for JS errors after interaction

```bash
chrome-devtools list_console_messages --types error
```

### Verify login flow

```bash
chrome-devtools take_snapshot
chrome-devtools fill <username_uid> "myuser"
chrome-devtools fill <password_uid> "pass123"
chrome-devtools click <signup_or_login_uid>
sleep 2
chrome-devtools take_snapshot
chrome-devtools list_console_messages --types error
```

---

## File Locations

| File                  | Purpose                                                   |
| --------------------- | --------------------------------------------------------- |
| `server/src/main.rs`  | Axum WS server, twig projection, updater                  |
| `shared/src/lib.rs`   | Shared types: `Op`, `FullStore`, `ServerMsg`, `ClientMsg` |
| `client/src/app.rs`   | WASM UI (respo components)                                |
| `client/src/store.rs` | WASM store + `ActionOp` dispatch                          |
| `client/dist/`        | Built static files served by server                       |
| `client/dist/pkg/`    | Compiled WASM + JS bindings                               |
| `test_board.js`       | CDP automation test (Node.js)                             |
