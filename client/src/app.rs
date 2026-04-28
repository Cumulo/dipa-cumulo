use std::cell::Ref;
use std::rc::Rc;

use respo::css::{
    respo_style, CssColor, CssBorderStyle, CssDisplay, CssFontWeight, CssFlexDirection,
    CssFlexJustifyContent, CssFlexAlignItems, CssPosition, CssSize,
};
use respo::states_tree::{RespoState, RespoStatesTree};
use respo::ui::{ui_button, ui_center, ui_global, ui_input};
use respo::{button, div, input, span, DispatchFn, RespoApp, RespoElement, RespoEvent, RespoNode, RespoStore};
use respo_state_derive::RespoState;
use serde::{Deserialize, Serialize};
use shared::{ChatPost, ClientStore, GlobalStats, OnlineUser, Op, PublicData};

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
        let public_data = &fs.public_data;

        if !cs.logged_in {
            let login_node = comp_login(&states.pick("login"), store.ws.as_ref())?;
            let messages = fs.base.messages.as_slice();
            if messages.is_empty() {
                return Ok(login_node);
            }
            return Ok(div()
                .children(vec![
                    login_node,
                    comp_messages(messages, store.ws.as_ref())?.to_node(),
                ])
                .to_node());
        }

        let user_data = fs.user_data.as_ref();
        let messages = fs.base.messages.as_slice();
        let todos = user_data.map(|u| u.todos.as_slice()).unwrap_or(&[]);
        let current_user = user_data.and_then(|u| u.user.as_ref());

        let page: RespoNode<ActionOp> = match cs.router.name.as_str() {
            "board" => comp_board(
                &public_data.chat_posts,
                current_user,
                &states.pick("board"),
                store.ws.as_ref(),
            )?
            .to_node(),
            "todos" => comp_todo_list(todos, &states.pick("todos"), store.ws.as_ref())?.to_node(),
            "profile" => comp_profile(
                current_user,
                &states.pick("profile"),
                store.ws.as_ref(),
            )?
            .to_node(),
            r if r.starts_with("user:") => {
                let uid = &r["user:".len()..];
                let viewed = public_data.online_users.iter().find(|u| u.id == uid);
                comp_user_profile_view(viewed)?.to_node()
            }
            _ => comp_home(&cs.global)?.to_node(),
        };

        Ok(div()
            .class(ui_global())
            .children(vec![
                comp_navigation(cs, public_data, store.ws.as_ref())?.to_node(),
                page,
                comp_messages(messages, store.ws.as_ref())?.to_node(),
            ])
            .to_node())
    }
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

fn nav_tab(
    label: &str,
    route: &'static str,
    active: bool,
    ws: Option<web_sys::WebSocket>,
) -> RespoNode<ActionOp> {
    button()
        .style(
            respo_style()
                .padding4(6, 14, 6, 14)
                .border_radius(4.0)
                .font_weight(if active { CssFontWeight::Weight(700) } else { CssFontWeight::Weight(400) })
                .background_color(if active { CssColor::Hsl(220, 70, 50) } else { CssColor::Hsl(0, 0, 94) })
                .color(if active { CssColor::White } else { CssColor::Hsl(0, 0, 30) })
                .cursor("pointer"),
        )
        .inner_text(label)
        .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
            // Apply route change immediately in local store (dipa cannot diff String in nested structs)
            dispatch.run(crate::store::ActionOp::RouteChange(route.into()))?;
            // Also notify the server for persistence
            if let Some(ws) = &ws {
                crate::dispatch_op(
                    &Store { ws: Some(ws.clone()), ..Default::default() },
                    Op::RouterChange { name: route.into() },
                );
            }
            Ok(())
        })
        .to_node()
}

fn comp_navigation(
    cs: &ClientStore,
    public_data: &PublicData,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    let route = cs.router.name.as_str();
    let g = &cs.global;

    // Online user badges — clickable, navigate to their read-only profile
    let mut right_children: Vec<RespoNode<ActionOp>> = vec![
        stat_badge(g.online_count, "online"),
        sep_dot(),
        stat_badge(g.total_users, "users"),
        sep_dot(),
        stat_badge(g.total_todos, "todos"),
    ];

    if !public_data.online_users.is_empty() {
        right_children.push(
            span()
                .style(respo_style().padding4(0, 10, 0, 10).color(CssColor::Hsl(0, 0, 78)))
                .inner_text("|")
                .to_node(),
        );
        for user in &public_data.online_users {
            let uid = user.id.clone();
            let route_target = format!("user:{}", uid);
            let is_active = route == route_target.as_str();
            right_children.push(
                button()
                    .style(
                        respo_style()
                            .padding4(3, 9, 3, 9)
                            .border_radius(12.0)
                            .font_size(12.0)
                            .cursor("pointer")
                            .margin4(0, 4, 0, 0)
                            .background_color(if is_active {
                                CssColor::Hsl(220, 70, 50)
                            } else {
                                CssColor::Hsl(220, 40, 92)
                            })
                            .color(if is_active {
                                CssColor::White
                            } else {
                                CssColor::Hsl(220, 50, 35)
                            }),
                    )
                    .inner_text(user.name.clone())
                    .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
                        dispatch.run(crate::store::ActionOp::RouteChange(route_target.clone()))?;
                        Ok(())
                    })
                    .to_node(),
            );
        }
    }

    Ok(div()
        .style(
            respo_style()
                .height(CssSize::Px(52.0))
                .display(CssDisplay::Flex)
                .align_items(CssFlexAlignItems::Center)
                .justify_content(CssFlexJustifyContent::SpaceBetween)
                .padding4(0, 16, 0, 16)
                .background_color(CssColor::White)
                .border(Some((1.0, CssBorderStyle::Solid, CssColor::Hsl(0, 0, 88)))),
        )
        .children(vec![
            // Left: brand + nav tabs
            div()
                .style(respo_style().display(CssDisplay::Flex).align_items(CssFlexAlignItems::Center))
                .children(vec![
                    span()
                        .style(respo_style().font_weight(CssFontWeight::Weight(700)).font_size(18.0).padding4(0, 16, 0, 0).color(CssColor::Hsl(220, 70, 45)))
                        .inner_text("Cumulo")
                        .to_node(),
                    nav_tab("Home",    "home",    route == "home" || route.is_empty(), ws.cloned()),
                    nav_tab("Board",   "board",   route == "board",   ws.cloned()),
                    nav_tab("Todos",   "todos",   route == "todos",   ws.cloned()),
                    nav_tab("Profile", "profile", route == "profile", ws.cloned()),
                ])
                .to_node(),
            // Right: stats + online user badges
            div()
                .style(
                    respo_style()
                        .display(CssDisplay::Flex)
                        .align_items(CssFlexAlignItems::Center)
                        .font_size(13.0)
                        .color(CssColor::Hsl(0, 0, 45)),
                )
                .children(right_children)
                .to_node(),
        ]))
}

fn stat_badge(n: u32, label: &str) -> RespoNode<ActionOp> {
    span()
        .children(vec![
            span()
                .style(respo_style().font_weight(CssFontWeight::Weight(700)).color(CssColor::Hsl(220, 60, 45)))
                .inner_text(n.to_string())
                .to_node(),
            span().inner_text(format!(" {}", label)).to_node(),
        ])
        .to_node()
}

fn sep_dot() -> RespoNode<ActionOp> {
    span()
        .style(respo_style().padding4(0, 6, 0, 6).color(CssColor::Hsl(0, 0, 75)))
        .inner_text("·")
        .to_node()
}

// ---------------------------------------------------------------------------
// Home page — shows architecture concepts + live global stats
// ---------------------------------------------------------------------------

fn comp_home(global: &GlobalStats) -> Result<RespoElement<ActionOp>, String> {
    Ok(div()
        .style(respo_style().padding(24).display(CssDisplay::Flex).flex_direction(CssFlexDirection::Column))
        .children(vec![
            div()
                .style(respo_style().font_size(26.0).font_weight(CssFontWeight::Weight(700)).padding4(0, 0, 8, 0))
                .children(vec![span().inner_text("Welcome to Cumulo").to_node()])
                .to_node(),
            div()
                .style(respo_style().font_size(14.0).color(CssColor::Hsl(0, 0, 45)).padding4(0, 0, 24, 0))
                .children(vec![span().inner_text("Server-managed state synced to clients via binary diffs (dipa).").to_node()])
                .to_node(),
            // Concept cards
            concept_card(
                "Global State",
                "Hsl(220, 70, 96)",
                &format!(
                    "{} users connected now · {} total users · {} todos in the system. \
                     Updated in real-time whenever anyone joins, leaves, or acts.",
                    global.online_count, global.total_users, global.total_todos
                ),
            ),
            concept_card(
                "Personal State",
                "Hsl(140, 60, 95)",
                "Your own todos and profile are visible only to you. \
                 Managed under the Todos and Profile tabs.",
            ),
            concept_card(
                "Local UI State",
                "Hsl(40, 80, 95)",
                "Text inputs, editing modes, etc. live entirely in the browser via \
                 respo's state tree — never sent to the server.",
            ),
            concept_card(
                "Delta Sync",
                "Hsl(280, 50, 95)",
                "The server computes a binary diff (dipa) between old and new state \
                 per session and sends only what changed over WebSocket.",
            ),
        ]))
}

fn concept_card(title: &str, _bg: &str, body: &str) -> RespoNode<ActionOp> {
    div()
        .style(
            respo_style()
                .background_color(CssColor::Hsl(210, 40, 97))
                .border_radius(8.0)
                .padding(16)
                .margin4(0, 0, 12, 0),
        )
        .children(vec![
            div()
                .style(respo_style().font_weight(CssFontWeight::Weight(700)).padding4(0, 0, 6, 0))
                .children(vec![span().inner_text(title).to_node()])
                .to_node(),
            div()
                .style(respo_style().font_size(14.0).color(CssColor::Hsl(0, 0, 40)))
                .children(vec![span().inner_text(body).to_node()])
                .to_node(),
        ])
        .to_node()
}

// ---------------------------------------------------------------------------
// Public board — shared chat visible to all logged-in users
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, RespoState)]
struct BoardState {
    text: String,
}

fn comp_board(
    posts: &[ChatPost],
    current_user: Option<&shared::UserView>,
    states: &RespoStatesTree,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    let state: Rc<BoardState> = states.cast_branch::<BoardState>();
    let cursor = states.path();

    let post_rows: Vec<RespoNode<ActionOp>> = posts
        .iter()
        .map(|post| {
            div()
                .style(
                    respo_style()
                        .padding4(10, 14, 10, 14)
                        .border_radius(6.0)
                        .background_color(CssColor::White)
                        .border(Some((1.0, CssBorderStyle::Solid, CssColor::Hsl(0, 0, 92))))
                        .margin4(0, 0, 8, 0)
                        .display(CssDisplay::Flex)
                        .align_items(CssFlexAlignItems::Baseline),
                )
                .children(vec![
                    span()
                        .style(
                            respo_style()
                                .font_weight(CssFontWeight::Weight(700))
                                .color(CssColor::Hsl(220, 60, 40))
                                .font_size(14.0)
                                .padding4(0, 10, 0, 0),
                        )
                        .inner_text(post.author_name.clone())
                        .to_node(),
                    span()
                        .style(respo_style().font_size(15.0).color(CssColor::Hsl(0, 0, 15)))
                        .inner_text(post.text.clone())
                        .to_node(),
                ])
                .to_node()
        })
        .collect();

    let cursor_input = cursor.clone();
    let cursor_submit = cursor.clone();
    let state_input = state.clone();
    let state_submit = state.clone();
    let ws_submit = ws.cloned();

    let mut children: Vec<RespoNode<ActionOp>> = vec![
        div()
            .style(
                respo_style()
                    .font_size(20.0)
                    .font_weight(CssFontWeight::Weight(700))
                    .padding4(0, 0, 16, 0),
            )
            .children(vec![span().inner_text("Public Board").to_node()])
            .to_node(),
        div()
            .style(respo_style().display(CssDisplay::Flex).padding4(0, 0, 16, 0))
            .children(vec![
                input()
                    .class(ui_input())
                    .attrs(&[
                        ("placeholder", "Say something to everyone\u{2026}"),
                        ("value", state.text.as_str()),
                    ])
                    .on_input(move |e: RespoEvent, dispatch: DispatchFn<_>| {
                        if let RespoEvent::Input { value, .. } = e {
                            let mut s: BoardState = (*state_input).clone();
                            s.text = value;
                            dispatch.run_state(&cursor_input, s)?;
                        }
                        Ok(())
                    })
                    .to_node(),
                button()
                    .class(ui_button())
                    .inner_text("Post")
                    .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
                        let text = state_submit.text.trim().to_string();
                        if !text.is_empty() {
                            if let Some(ws) = &ws_submit {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::PostChat { text },
                                );
                            }
                            dispatch.run_state(&cursor_submit, BoardState::default())?;
                        }
                        Ok(())
                    })
                    .to_node(),
            ])
            .to_node(),
    ];

    if post_rows.is_empty() {
        children.push(
            div()
                .style(
                    respo_style()
                        .font_size(14.0)
                        .color(CssColor::Hsl(0, 0, 60))
                        .padding4(16, 0, 16, 0),
                )
                .children(vec![span().inner_text("No posts yet. Be the first to say hi!").to_node()])
                .to_node(),
        );
    } else {
        children.extend(post_rows);
    }

    let _ = current_user;
    Ok(div()
        .style(
            respo_style()
                .padding(24)
                .display(CssDisplay::Flex)
                .flex_direction(CssFlexDirection::Column),
        )
        .children(children))
}

// ---------------------------------------------------------------------------
// Read-only user profile — view another user's public info
// ---------------------------------------------------------------------------

fn comp_user_profile_view(user: Option<&OnlineUser>) -> Result<RespoElement<ActionOp>, String> {
    let back_btn = button()
        .class(ui_button())
        .style(respo_style().margin4(24, 0, 0, 0))
        .inner_text("\u{2190} Back")
        .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
            dispatch.run(crate::store::ActionOp::RouteChange("home".to_string()))?;
            Ok(())
        })
        .to_node();

    match user {
        None => Ok(div()
            .style(
                respo_style()
                    .padding(24)
                    .display(CssDisplay::Flex)
                    .flex_direction(CssFlexDirection::Column),
            )
            .children(vec![
                div()
                    .style(respo_style().font_size(16.0).color(CssColor::Hsl(0, 0, 50)).padding4(0, 0, 16, 0))
                    .children(vec![span().inner_text("User not found or went offline.").to_node()])
                    .to_node(),
                back_btn,
            ])),
        Some(u) => {
            let bio_text =
                if u.bio.is_empty() { "(no bio yet)".to_string() } else { u.bio.clone() };
            Ok(div()
                .style(
                    respo_style()
                        .padding(24)
                        .display(CssDisplay::Flex)
                        .flex_direction(CssFlexDirection::Column),
                )
                .children(vec![
                    div()
                        .style(
                            respo_style()
                                .font_size(26.0)
                                .font_weight(CssFontWeight::Weight(300))
                                .padding4(0, 0, 4, 0),
                        )
                        .children(vec![span().inner_text(u.name.clone()).to_node()])
                        .to_node(),
                    div()
                        .style(
                            respo_style()
                                .font_size(12.0)
                                .color(CssColor::Hsl(140, 50, 38))
                                .padding4(0, 0, 24, 0),
                        )
                        .children(vec![span().inner_text("\u{25cf} Online now").to_node()])
                        .to_node(),
                    div()
                        .style(respo_style().font_weight(CssFontWeight::Weight(700)).padding4(0, 0, 8, 0))
                        .children(vec![span().inner_text("Bio").to_node()])
                        .to_node(),
                    div()
                        .style(respo_style().font_size(15.0).color(CssColor::Hsl(0, 0, 30)))
                        .children(vec![span().inner_text(bio_text).to_node()])
                        .to_node(),
                    back_btn,
                ]))
        }
    }
}

// ---------------------------------------------------------------------------
// Todo list — personal data + local UI state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, RespoState)]
struct TodoInputState {
    text: String,
}

fn comp_todo_list(
    todos: &[shared::TodoItem],
    states: &RespoStatesTree,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    let state: Rc<TodoInputState> = states.cast_branch::<TodoInputState>();
    let cursor = states.path();

    let total = todos.len();
    let done = todos.iter().filter(|t| t.completed).count();

    // Build todo rows
    let todo_rows: Vec<RespoNode<ActionOp>> = todos
        .iter()
        .map(|todo| {
            let id_toggle = todo.id.clone();
            let id_delete = todo.id.clone();
            let ws_toggle = ws.cloned();
            let ws_delete = ws.cloned();
            let completed = todo.completed;

            div()
                .style(
                    respo_style()
                        .display(CssDisplay::Flex)
                        .align_items(CssFlexAlignItems::Center)
                        .padding4(10, 0, 10, 0)
                        .border(Some((1.0, CssBorderStyle::Solid, CssColor::Hsl(0, 0, 92))))
                        .border_radius(6.0)
                        .background_color(if completed { CssColor::Hsl(0, 0, 97) } else { CssColor::White })
                        .margin4(0, 0, 6, 0)
                        .padding4(10, 12, 10, 12),
                )
                .children(vec![
                    // Toggle button
                    button()
                        .style(
                            respo_style()
                                .width(CssSize::Px(22.0))
                                .height(CssSize::Px(22.0))
                                .border_radius(11.0)
                                .background_color(if completed {
                                    CssColor::Hsl(140, 60, 48)
                                } else {
                                    CssColor::Hsl(0, 0, 88)
                                })
                                .color(CssColor::White)
                                .font_size(12.0)
                                .cursor("pointer"),
                        )
                        .inner_text(if completed { "✓" } else { "" })
                        .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                            if let Some(ws) = &ws_toggle {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::ToggleTodo { id: id_toggle.clone() },
                                );
                            }
                            Ok(())
                        })
                        .to_node(),
                    // Text
                    span()
                        .style(
                            respo_style()
                                .padding4(0, 0, 0, 10)
                                .opacity(if completed { 0.4 } else { 1.0 })
                                .font_size(15.0),
                        )
                        .inner_text(todo.text.clone())
                        .to_node(),
                    // Delete button — pushed to end via spacer trick using padding on the span
                    button()
                        .style(
                            respo_style()
                                .color(CssColor::Hsl(0, 60, 55))
                                .padding4(2, 6, 2, 6)
                                .font_size(16.0)
                                .cursor("pointer"),
                        )
                        .inner_text("×")
                        .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                            if let Some(ws) = &ws_delete {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::DeleteTodo { id: id_delete.clone() },
                                );
                            }
                            Ok(())
                        })
                        .to_node(),
                ])
                .to_node()
        })
        .collect();

    let cursor_input = cursor.clone();
    let cursor_add = cursor.clone();
    let state_input = state.clone();
    let state_add = state.clone();
    let ws_add = ws.cloned();

    let mut children: Vec<RespoNode<ActionOp>> = vec![
        // Header
        div()
            .style(respo_style().font_size(20.0).font_weight(CssFontWeight::Weight(700)).padding4(0, 0, 16, 0))
            .children(vec![span().inner_text("My Todos").to_node()])
            .to_node(),
        // Input row
        div()
            .style(respo_style().display(CssDisplay::Flex).padding4(0, 0, 16, 0))
            .children(vec![
                input()
                    .class(ui_input())
                    .attrs(&[("placeholder", "What needs to be done?"), ("value", state.text.as_str())])
                    .on_input({
                        let cursor = cursor_input;
                        let state = state_input;
                        move |e: RespoEvent, dispatch: DispatchFn<_>| {
                            if let RespoEvent::Input { value, .. } = e {
                                let mut s: TodoInputState = (*state).clone();
                                s.text = value;
                                dispatch.run_state(&cursor, s)?;
                            }
                            Ok(())
                        }
                    })
                    .to_node(),
                button()
                    .class(ui_button())
                    .inner_text("Add")
                    .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
                        let text = state_add.text.trim().to_string();
                        if !text.is_empty() {
                            if let Some(ws) = &ws_add {
                                crate::dispatch_op(
                                    &Store { ws: Some(ws.clone()), ..Default::default() },
                                    Op::AddTodo { text },
                                );
                            }
                            dispatch.run_state(&cursor_add, TodoInputState::default())?;
                        }
                        Ok(())
                    })
                    .to_node(),
            ])
            .to_node(),
    ];

    children.extend(todo_rows);

    // Footer: completion summary
    children.push(
        div()
            .style(respo_style().padding4(12, 0, 0, 0).font_size(13.0).color(CssColor::Hsl(0, 0, 50)))
            .children(vec![
                span().inner_text(format!("{} / {} completed", done, total)).to_node(),
            ])
            .to_node(),
    );

    Ok(div()
        .style(respo_style().padding(24).display(CssDisplay::Flex).flex_direction(CssFlexDirection::Column))
        .children(children))
}

// ---------------------------------------------------------------------------
// Profile — personal data + local editing state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, RespoState)]
struct ProfileState {
    bio_draft: String,
    editing: bool,
}

fn comp_profile(
    user: Option<&shared::UserView>,
    states: &RespoStatesTree,
    ws: Option<&web_sys::WebSocket>,
) -> Result<RespoElement<ActionOp>, String> {
    let state: Rc<ProfileState> = states.cast_branch::<ProfileState>();
    let cursor = states.path();

    let name = user.map(|u| u.name.as_str()).unwrap_or("Unknown").to_owned();
    let bio = user.map(|u| u.bio.as_str()).unwrap_or("").to_owned();

    let bio_section: RespoElement<ActionOp> = if state.editing {
        let cursor_input = cursor.clone();
        let cursor_save = cursor.clone();
        let state_input = state.clone();
        let state_save = state.clone();
        let ws_save = ws.cloned();

        div()
            .style(respo_style().display(CssDisplay::Flex).align_items(CssFlexAlignItems::Center))
            .children(vec![
                input()
                    .class(ui_input())
                    .attrs(&[("placeholder", "Write something about yourself…"), ("value", state.bio_draft.as_str())])
                    .on_input(move |e: RespoEvent, dispatch: DispatchFn<_>| {
                        if let RespoEvent::Input { value, .. } = e {
                            let mut s: ProfileState = (*state_input).clone();
                            s.bio_draft = value;
                            dispatch.run_state(&cursor_input, s)?;
                        }
                        Ok(())
                    })
                    .to_node(),
                button()
                    .class(ui_button())
                    .inner_text("Save")
                    .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
                        if let Some(ws) = &ws_save {
                            crate::dispatch_op(
                                &Store { ws: Some(ws.clone()), ..Default::default() },
                                Op::UpdateBio { bio: state_save.bio_draft.clone() },
                            );
                        }
                        let mut s: ProfileState = (*state_save).clone();
                        s.editing = false;
                        dispatch.run_state(&cursor_save, s)?;
                        Ok(())
                    })
                    .to_node(),
            ])
    } else {
        let cursor_edit = cursor.clone();
        let _state_edit = state.clone();
        let bio_for_draft = bio.clone();

        div()
            .style(respo_style().display(CssDisplay::Flex).align_items(CssFlexAlignItems::Center))
            .children(vec![
                span()
                    .style(respo_style().padding4(0, 12, 0, 0).color(if bio.is_empty() {
                        CssColor::Hsl(0, 0, 65)
                    } else {
                        CssColor::Hsl(0, 0, 20)
                    }))
                    .inner_text(if bio.is_empty() { "(no bio yet)" } else { &bio })
                    .to_node(),
                button()
                    .class(ui_button())
                    .inner_text("Edit")
                    .on_click(move |_e: RespoEvent, dispatch: DispatchFn<_>| {
                        let s = ProfileState { editing: true, bio_draft: bio_for_draft.clone() };
                        dispatch.run_state(&cursor_edit, s)?;
                        Ok(())
                    })
                    .to_node(),
            ])
    };

    let ws_logout = ws.cloned();

    Ok(div()
        .style(respo_style().padding(24).display(CssDisplay::Flex).flex_direction(CssFlexDirection::Column))
        .children(vec![
            div()
                .style(respo_style().font_size(26.0).font_weight(CssFontWeight::Weight(300)).padding4(0, 0, 24, 0))
                .children(vec![span().inner_text(format!("Hello, {}!", name)).to_node()])
                .to_node(),
            div()
                .style(respo_style().font_weight(CssFontWeight::Weight(700)).padding4(0, 0, 8, 0))
                .children(vec![span().inner_text("Bio").to_node()])
                .to_node(),
            bio_section.to_node(),
            button()
                .class(ui_button())
                .style(respo_style().margin4(32, 0, 0, 0))
                .inner_text("Log out")
                .on_click(move |_e: RespoEvent, _dispatch: DispatchFn<_>| {
                    if let Some(ws) = &ws_logout {
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
