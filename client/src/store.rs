use respo::states_tree::{RespoStatesTree, RespoUpdateState};
use respo::{RespoAction, RespoStore};
use serde::{Deserialize, Serialize};
use shared::FullStore;
use web_sys::WebSocket;

// ---------------------------------------------------------------------------
// App store — holds both local respo state and the server-synced FullStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Store {
    pub full_store: FullStore,
    pub states: RespoStatesTree,
    /// WebSocket handle — not serialized
    #[serde(skip)]
    pub ws: Option<WebSocket>,
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub enum ActionOp {
    #[default]
    Noop,
    StatesChange(RespoUpdateState),
    /// Server acknowledged a snapshot — replace local full_store
    ServerSnapshot(FullStore),
    /// Server sent a patch — applied externally, triggers re-render via Noop
    TriggerRerender,
}

impl RespoAction for ActionOp {
    type Intent = ();
    fn states_action(a: RespoUpdateState) -> Self {
        Self::StatesChange(a)
    }
}

impl RespoStore for Store {
    type Action = ActionOp;

    fn get_states(&mut self) -> &mut RespoStatesTree {
        &mut self.states
    }

    fn update(&mut self, op: Self::Action) -> Result<(), String> {
        match op {
            ActionOp::Noop | ActionOp::TriggerRerender => {}
            ActionOp::StatesChange(a) => self.update_states(a),
            ActionOp::ServerSnapshot(snapshot) => {
                self.full_store = snapshot;
            }
        }
        Ok(())
    }

    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn try_from_string(s: &str) -> Result<Self, String>
    where
        Self: Sized,
    {
        serde_json::from_str(s).map_err(|e| format!("parse store: {e}"))
    }
}

