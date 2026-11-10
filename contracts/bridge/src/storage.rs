use soroban_sdk::{contracttype, Env, String};

use crate::types::{AgentAccount, BridgeState, ConversionRecord};

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

/// Ledgers are roughly 5 seconds apart, so one day is about 17,280 ledgers.
const LEDGER_THRESHOLD: u32 = 17_280 * 30;
const LEDGER_BUMP: u32 = 17_280 * 120;

/// Read the BridgeState singleton, if initialized.
pub fn get_state(env: &Env) -> Option<BridgeState> {
    env.storage().instance().get(&DataKey::State)
}

/// Write the BridgeState singleton.
pub fn set_state(env: &Env, state: &BridgeState) {
    env.storage().instance().set(&DataKey::State, state);
    env.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
}

pub fn get_agent(env: &Env, agent_id: &String) -> Option<AgentAccount> {
    env.storage()
        .persistent()
        .get(&DataKey::Agent(agent_id.clone()))
}

pub fn set_agent(env: &Env, agent_id: &String, agent: &AgentAccount) {
    let key = DataKey::Agent(agent_id.clone());
    env.storage().persistent().set(&key, agent);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
}

pub fn get_conversion(env: &Env, conversion_id: &String) -> Option<ConversionRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::Conversion(conversion_id.clone()))
}

pub fn set_conversion(env: &Env, conversion_id: &String, record: &ConversionRecord) {
    let key = DataKey::Conversion(conversion_id.clone());
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
}

/// USDC credited to an agent on a given day (epoch day). Returns 0 when unset.
pub fn get_daily_total(env: &Env, agent_id: &String, day: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::DailyTotal(agent_id.clone(), day))
        .unwrap_or(0)
}

pub fn set_daily_total(env: &Env, agent_id: &String, day: u64, total: i128) {
    let key = DataKey::DailyTotal(agent_id.clone(), day);
    env.storage().persistent().set(&key, &total);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
}
