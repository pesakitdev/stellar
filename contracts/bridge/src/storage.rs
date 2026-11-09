use soroban_sdk::{contracttype, Env, String};

use crate::types::BridgeState;

#[contracttype]
pub enum DataKey {
    /// BridgeState singleton.
    State,
    /// AgentAccount keyed by agent_id string.
    Agent(String),
    /// ConversionRecord keyed by conversion_id string.
    Conversion(String),
    /// Daily conversion total keyed by (agent_id, date_epoch_day: u64).
    /// date_epoch_day = timestamp_seconds / 86400
    DailyTotal(String, u64),
}

/// Read the BridgeState singleton, if initialized.
pub fn get_state(env: &Env) -> Option<BridgeState> {
    env.storage().instance().get(&DataKey::State)
}

/// Write the BridgeState singleton.
pub fn set_state(env: &Env, state: &BridgeState) {
    env.storage().instance().set(&DataKey::State, state);
}
