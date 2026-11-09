use soroban_sdk::{Address, Env, String, Symbol};

/// Emitted on every successful conversion. This is the on-chain metric
/// tracked for the SCF milestone: cumulative USDC = sum of usdc_amount
/// across all ConversionCompleted events on this contract.
pub fn emit_conversion_completed(
    env: &Env,
    conversion_id: String,
    agent_id: String,
    stellar_address: Address,
    kes_amount: i128,
    usdc_amount: i128,
) {
    env.events().publish(
        (Symbol::new(env, "ConversionCompleted"), stellar_address.clone()),
        (conversion_id, agent_id, stellar_address, kes_amount, usdc_amount),
    );
}

pub fn emit_bridge_paused(env: &Env, by: Address) {
    env.events().publish((Symbol::new(env, "BridgePaused"),), (by,));
}

pub fn emit_bridge_unpaused(env: &Env, by: Address) {
    env.events().publish((Symbol::new(env, "BridgeUnpaused"),), (by,));
}

pub fn emit_agent_registered(env: &Env, agent_id: String, stellar_address: Address) {
    env.events().publish(
        (Symbol::new(env, "AgentRegistered"),),
        (agent_id, stellar_address),
    );
}
