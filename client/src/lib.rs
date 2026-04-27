mod app;
pub(crate) mod store;

use std::cell::RefCell;
use std::panic;
use std::rc::Rc;

use dipa::Patchable;
use respo::{util, RespoApp};
use shared::{ClientMsg, FullStore, Op, ServerMsg};
use store::Store;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, MessageEvent, WebSocket};

/// Send an Op to the server via the WebSocket stored in the store
pub fn dispatch_op(store: &Store, op: Op) {
    if let Some(ws) = &store.ws {
        let msg = ClientMsg::Op(op);
        if let Ok(bytes) = bincode::serialize(&msg) {
            let array = js_sys::Uint8Array::from(bytes.as_slice());
            let _ = ws.send_with_array_buffer_view(&array);
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    let app = app::App {
        mount_target: util::query_select_node(".app").expect("mount target"),
        store: Rc::new(RefCell::new(Store::default())),
    };

    connect_ws(app.get_store().clone());

    app.render_loop().expect("app render");
}

fn connect_ws(store_rc: Rc<RefCell<Store>>) {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let host = location.hostname().unwrap_or_default();
    let port = "5021";
    let url = format!("ws://{}:{}/ws", host, port);

    let ws = WebSocket::new(&url).expect("failed to create WebSocket");
    ws.set_binary_type(BinaryType::Arraybuffer);

    // on_message: deserialize server frames, apply to store, request rerender
    let store_msg = store_rc.clone();
    let on_message = Closure::<dyn Fn(MessageEvent)>::new(move |e: MessageEvent| {
        let data = e.data();
        let buf: js_sys::ArrayBuffer = match data.dyn_into() {
            Ok(b) => b,
            Err(_) => return,
        };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        let msg: ServerMsg = match bincode::deserialize(&bytes) {
            Ok(m) => m,
            Err(e) => {
                util::error_log!("deserialize server msg: {e}");
                return;
            }
        };

        match msg {
            ServerMsg::Snapshot(snapshot) => {
                let mut s = store_msg.borrow_mut();
                s.full_store = snapshot;
            }
            ServerMsg::Patch(patch_bytes) => {
                type Delta = <FullStore as dipa::Diffable<'static, 'static, FullStore>>::DeltaOwned;
                match bincode::deserialize::<Delta>(&patch_bytes) {
                    Ok(delta) => {
                        store_msg.borrow_mut().full_store.apply_patch(delta);
                    }
                    Err(e) => {
                        util::error_log!("deserialize patch: {e}");
                        return;
                    }
                }
            }
            ServerMsg::Pong => {}
        }

        // Trigger respo re-render
        respo::request_rerender();
    });
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let on_error = Closure::<dyn Fn()>::new(|| {
        util::error_log!("WebSocket error");
    });
    ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    on_error.forget();

    // Store ws handle in store
    store_rc.borrow_mut().ws = Some(ws);
}

