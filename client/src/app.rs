use std::cell::Ref;
use std::rc::Rc;

use respo::css::{respo_style, CssColor, CssDisplay, CssFontWeight, CssFlexDirection,
    CssFlexJustifyContent, CssFlexAlignItems, CssPosition, CssSize};
use respo::states_tree::{RespoState, RespoStatesTree};
use respo::ui::{ui_button, ui_center, ui_global, ui_input};
use respo::{button, div, input, span, DispatchFn, RespoApp, RespoElement, RespoEvent, RespoNode, RespoStore};
use respo_state_derive::RespoState;
use serde::{Deserialize, Serialize};
use shared::Op;

use crate::store::{ActionOp, Store};

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub store: Rc<std::cell::RefCell<Store>>,
    pub mount_target: web_sys::Node,
}

impl RespoApp for App {
    type Model = Store;

    fn get_store(&self) -> &Rc<std::cell::RefCell<Self::Model>> {
        &self.store
    }

    fn get_mount_target(&self) -> &web_sys::Node {
        &self.mount_target
    }

    fn dispatch(
        store_rc: Rc<std::cell::RefCell<Self::Model>>,
        op: ActionOp,
    ) -> Result<(), String> {
        store_rc.borrow_mut().update(op)
    }

    fn view(store: Ref<Self::Model>) -> Result<RespoNode<ActionOp>, String> {
        let states = &store.states;
        let fs = &store.full_store;
        let cs = &fs.base;

        if !cs.logged_in {
            return comp_login(&states.pick("login"), store.ws.as_ref());
        }

        let user_data = fs.user_data.as_ref();
        let messages = user_data.map(|u| u.messages.as_slice()).unwrap_or(&[]);

        let page: RespoNode<ActionOp> = match cs.router.name.as_str() {
            "profile" => comp_profile(user_data.and_then(|u| u.user.as_ref()), store.ws.as_ref())?.to_node(),
            _ => comp_home()?.to_node(),
        };

        Ok(div()
            .class(ui_global())
            .children(vec![
                comp_navigation(cs.member_count)?.to_node(),
                page,
                comp_messages(messages, store.ws.as_ref())?.to_node(),
            ])
            .to_node())
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

fn comp_navigation(member_count: u32) -> Result<RespoElement<ActionOp>, String> {
    Ok(div()
        .style(
            respo_style()
                .height(CssSize::Px(48.0))
                .display(CssDisplay::Flex)
                .align_items(CssFlexAlignItems::Center)
                .justify_content(CssFlexJustifyContent::SpaceBetween)
                .padding4(0, 16, 0, 16),
        )
        .children(vec![
            span().inner_text("Calcium").to_node(),
            span().inner_text(format!("{} online", member_count)).to_node(),
        ]))
}

fn comp_home() -> Result<RespoElement<ActionOp>, String> {
    Ok(div()
        .style(respo_style().padding(16))
        .children(vec![span().inner_text("Home page").to_node()]))
}

fn comp_profile(
    user: Option<&shared::UserView>,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    let name = user.map(|u| u.name.as_str()).unwrap_or("Unknown").to_owned();
    let ws_cloned = ws.cloned();
    Ok(div()
        .style(
            respo_style()
                .padding(16)
                .display(CssDisplay::Flex)
                .flex_direction(CssFlexDirection::Column),
        )
        .children(vec![
            div()
                .style(respo_style().font_size(32.0).font_weight(CssFontWeight::Weight(100)).margin4(0, 0, 16, 0))
                .children(vec![span().inner_text(format!("Hello! {}", name)).to_node()])
                .to_node(),
            button()
                .class(ui_button())
                .inner_text("Log out")
                .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                    if let Some(ws) = &ws_cloned {
                        crate::dispatch_op(
                            &Store { ws: Some(ws.clone()), ..Default::default() },
                            Op::UserLogOut,
                        );
                    }
                    Ok(())
                })
                .to_node(),
        ]))
}

// ---------------------------------------------------------------------------
// Login component with local state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, RespoState)]
struct LoginState {
    username: String,
    password: String,
}

fn comp_login(
    states: &RespoStatesTree,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoNode<ActionOp>, String> {
    let state: Rc<LoginState> = states.cast_branch::<LoginState>();
    let cursor = states.path();

    let ws_login = ws.cloned();
    let ws_signup = ws.cloned();
    let state_login = state.clone();
    let state_signup = state.clone();

    Ok(div()
        .class(ui_center())
        .style(respo_style().flex_direction(CssFlexDirection::Column))
        .children(vec![
            input()
                .class(ui_input())
                .attrs(&[("placeholder", "username"), ("value", state.username.as_str())])
                .on_input({
                    let cursor = cursor.clone();
                    let state = state.clone();
                    move |e: RespoEvent, dispatch: DispatchFn<_>| {
                        if let RespoEvent::Input { value, .. } = e {
                            let mut s: LoginState = (*state).clone();
                            s.username = value;
                            dispatch.run_state(&cursor, s)?;
                        }
                        Ok(())
                    }
                })
                .to_node(),
            input()
                .class(ui_input())
                .attrs(&[("placeholder", "password"), ("type", "password"), ("value", state.password.as_str())])
                .on_input({
                    let cursor = cursor.clone();
                    let state = state.clone();
                    move |e: RespoEvent, dispatch: DispatchFn<_>| {
                        if let RespoEvent::Input { value, .. } = e {
                            let mut s: LoginState = (*state).clone();
                            s.password = value;
                            dispatch.run_state(&cursor, s)?;
                        }
                        Ok(())
                    }
                })
                .to_node(),
            div()
                .style(respo_style().display(CssDisplay::Flex))
                .children(vec![
                    button()
                        .class(ui_button())
                        .inner_text("Log in")
                        .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                            if let Some(ws) = &ws_login {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::UserLogin {
                                        username: state_login.username.clone(),
                                        password: state_login.password.clone(),
                                    },
                                );
                            }
                            Ok(())
                        })
                        .to_node(),
                    button()
                        .class(ui_button())
                        .inner_text("Sign up")
                        .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                            if let Some(ws) = &ws_signup {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::UserSignUp {
                                        username: state_signup.username.clone(),
                                        password: state_signup.password.clone(),
                                    },
                                );
                            }
                            Ok(())
                        })
                        .to_node(),
                ])
                .to_node(),
        ])
        .to_node())
}

// ---------------------------------------------------------------------------
// Message toast component
// ---------------------------------------------------------------------------

fn comp_messages(
    messages: &[shared::Message],
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    if messages.is_empty() {
        return Ok(span());
    }
    let items: Vec<RespoNode<ActionOp>> = messages
        .iter()
        .map(|msg| {
            let id = msg.id.clone();
            let ws_cloned = ws.cloned();
            div()
                .style(
                    respo_style()
                        .padding4(8, 12, 8, 12)
                        .background_color(CssColor::Hsl(0, 80, 90))
                        .border_radius(4.0)
                        .display(CssDisplay::Flex)
                        .align_items(CssFlexAlignItems::Center),
                )
                .children(vec![
                    span().inner_text(msg.text.clone()).to_node(),
                    button()
                        .inner_text("×")
                        .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                            if let Some(ws) = &ws_cloned {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::SessionRemoveMessage { id: id.clone() },
                                );
                            }
                            Ok(())
                        })
                        .to_node(),
                ])
                .to_node()
        })
        .collect();

    Ok(div()
        .style(
            respo_style()
                .position(CssPosition::Fixed)
                .bottom(CssSize::Px(16.0))
                .right(CssSize::Px(16.0))
                .display(CssDisplay::Flex)
                .flex_direction(CssFlexDirection::Column),
        )
        .children(items))
}
