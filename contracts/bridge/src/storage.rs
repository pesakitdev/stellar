use soroban_sdk::{contracttype, Env, String};

use crate::types::{AgentAccount, BridgeState, ConversionRecord, ReturnRecord};

#[contracttype]
pub enum DataKey {
    /// BridgeState singleton.
    State,
    /// AgentAccount keyed by agent_id string.
    Agent(String),
    /// ConversionRecord keyed by conversion_id string.
    Conversion(String),
    /// Daily conversion total keyed by (agent_id, date_epoch_day: u64).
    /// date_epoch_day = (timestamp_seconds + 3 hours) / 86400, so the day changes at midnight Nairobi time
    DailyTotal(String, u64),
    /// ReturnRecord keyed by return_id string.
    Return(String),
    /// USDC returned by an agent on a day, keyed like DailyTotal. Counted separately
    /// from conversions, so each direction has its own daily limit.
    DailyReturned(String, u64),
    /// Cumulative USDC returned to the reserve.
    TotalReturned,
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

pub fn get_return(env: &Env, return_id: &String) -> Option<ReturnRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::Return(return_id.clone()))
}

pub fn set_return(env: &Env, return_id: &String, record: &ReturnRecord) {
    let key = DataKey::Return(return_id.clone());
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
}

/// USDC an agent returned on a given day. Returns 0 when unset.
pub fn get_daily_returned(env: &Env, agent_id: &String, day: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::DailyReturned(agent_id.clone(), day))
        .unwrap_or(0)
}

pub fn set_daily_returned(env: &Env, agent_id: &String, day: u64, total: i128) {
    let key = DataKey::DailyReturned(agent_id.clone(), day);
    env.storage().persistent().set(&key, &total);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);
}

/// Cumulative USDC returned to the reserve. Returns 0 when unset.
pub fn get_total_returned(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalReturned)
        .unwrap_or(0)
}

pub fn set_total_returned(env: &Env, total: i128) {
    env.storage().instance().set(&DataKey::TotalReturned, &total);
    env.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
}
