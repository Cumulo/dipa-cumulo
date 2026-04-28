# dipa-cumulo

A Rust reimplementation of the [calcium-workflow](https://github.com/Cumulo/calcium-workflow) architecture:
**server ↔ client** state sync via **diff/patch over WebSocket**, using [`dipa`](https://github.com/chinedufn/dipa) for delta encoding and [`respo.rs`](https://github.com/Respo/respo.rs) for WASM UI rendering.

## Architecture

```
shared/     — shared data types (ClientStore, Op, ServerMsg …)
             derive DiffPatch + Serde on all state structs

server/     — Tokio + Axum WebSocket server
             • maintains AppDb (users + sessions) in memory
             • updater(db, op) → new_db   (pure function, no side effects)
             • twig_container(db, session) → ClientStore   (projection per session)
             • diffs old vs new ClientStore with dipa, sends patch bytes

client/     — wasm-pack WASM frontend (respo.rs)
             • receives Snapshot on connect, applies Patch on each change
             • dipa::Patchable::apply_patch() updates local ClientStore in-place
             • respo.rs renders virtual DOM from the store
```

## Quick Start

### Prerequisites

```bash
cargo install wasm-pack
```

### Build client WASM

```bash
cd client
wasm-pack build --target web --out-dir dist/pkg
```

### Run server

```bash
cd server
cargo run --release
```

Then open `http://localhost:5021` in your browser.

## Key Design Decisions

| Concern      | Calcit solution                | Rust solution                              |
| ------------ | ------------------------------ | ------------------------------------------ |
| Diff/patch   | `recollect` (custom trie diff) | `dipa` (`#[derive(DiffPatch)]`)            |
| Transport    | `ws-edn` (EDN over WS)         | `bincode` (binary) over WebSocket          |
| UI           | Respo (ClojureScript)          | `respo.rs` (WASM)                          |
| Server       | Calcit server + Node WSS       | Axum + Tokio                               |
| Pure updater | `defn updater (db op …) → db`  | `fn updater(db: AppDb, op: Op, …) → AppDb` |
