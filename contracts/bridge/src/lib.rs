#![no_std]

mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String};

use crate::types::{AgentAccount, BridgeError, BridgeState, ConversionRecord};

/// Seconds in one calendar day. Daily limits reset at UTC midnight.
const SECONDS_PER_DAY: u64 = 86_400;
/// conversion_id is generated off-chain and may be at most 64 characters.
const MAX_CONVERSION_ID_LEN: u32 = 64;
/// agent_id is PesaKit's internal ID and may be at most 32 characters.
const MAX_AGENT_ID_LEN: u32 = 32;

fn day_of(timestamp: u64) -> u64 {
    timestamp / SECONDS_PER_DAY
}

fn load_state(env: &Env) -> Result<BridgeState, BridgeError> {
    storage::get_state(env).ok_or(BridgeError::NotInitialized)
}

/// Checks that an ID is non-empty and within `max_len` characters.
fn check_id(id: &String, max_len: u32) -> Result<(), BridgeError> {
    if id.len() == 0 || id.len() > max_len {
        return Err(BridgeError::InvalidId);
    }
    Ok(())
}

#[contract]
pub struct PesaKitBridge;

#[contractimpl]
impl PesaKitBridge {
    /// One-time initialization. Must be called by the deployer before any other function.
    /// Sets admin, USDC token address, reserve account, and initial limits.
    pub fn initialize(
        env: Env,
        admin: Address,
        usdc_token: Address,
        reserve_account: Address,
        max_per_tx_usdc: i128,  // e.g. 100_0000000 for $100 (7 decimal places)
        max_per_day_usdc: i128, // e.g. 750_0000000 for ~$750 (KSh 100,000)
    ) -> Result<(), BridgeError> {
        if storage::get_state(&env).is_some() {
            return Err(BridgeError::AlreadyInitialized);
        }
        if max_per_tx_usdc <= 0 || max_per_day_usdc <= 0 {
            return Err(BridgeError::InvalidAmount);
        }
        admin.require_auth();

        storage::set_state(
            &env,
            &BridgeState {
                admin,
                usdc_token,
                reserve_account,
                max_per_tx: max_per_tx_usdc,
                max_per_day: max_per_day_usdc,
                paused: false,
                total_converted: 0,
            },
        );
        Ok(())
    }

    /// Register a new agent. Called by admin before the agent's first conversion.
    /// agent_id: PesaKit internal ID (max 32 chars). stellar_address: recipient.
    pub fn register_agent(
        env: Env,
        agent_id: String,
        stellar_address: Address,
    ) -> Result<(), BridgeError> {
        let state = load_state(&env)?;
        state.admin.require_auth();
        check_id(&agent_id, MAX_AGENT_ID_LEN)?;

        if storage::get_agent(&env, &agent_id).is_some() {
            return Err(BridgeError::AgentAlreadyRegistered);
        }

        storage::set_agent(
            &env,
            &agent_id,
            &AgentAccount {
                stellar_address: stellar_address.clone(),
                lifetime_converted: 0,
                today_converted: 0,
                last_conversion_ts: 0,
                conversion_count: 0,
            },
        );
        events::emit_agent_registered(&env, agent_id, stellar_address);
        Ok(())
    }

    /// Execute a KES → USDC conversion for a registered agent.
    /// Called by the Stellar Service backend (admin-authorized caller).
    /// Checks limits, transfers USDC from reserve to agent, emits ConversionCompleted.
    /// conversion_id must be unique; duplicate IDs are rejected with DuplicateConversionId.
    pub fn convert(
        env: Env,
        conversion_id: String,
        agent_id: String,
        kes_amount: i128,
        usdc_amount: i128,
    ) -> Result<ConversionRecord, BridgeError> {
        let mut state = load_state(&env)?;
        state.admin.require_auth();
        // The reserve must authorize each conversion at the top level. Its USDC
        // transfer is a nested call, and Soroban only accepts that nested
        // authorization when the root call is authorized too. In production the
        // reserve signers provide this authorization.
        state.reserve_account.require_auth();

        if state.paused {
            return Err(BridgeError::BridgePaused);
        }
        check_id(&conversion_id, MAX_CONVERSION_ID_LEN)?;
        check_id(&agent_id, MAX_AGENT_ID_LEN)?;
        if storage::get_conversion(&env, &conversion_id).is_some() {
            return Err(BridgeError::DuplicateConversionId);
        }
        let mut agent =
            storage::get_agent(&env, &agent_id).ok_or(BridgeError::AgentNotRegistered)?;

        if kes_amount <= 0 || usdc_amount <= 0 {
            return Err(BridgeError::InvalidAmount);
        }
        if usdc_amount > state.max_per_tx {
            return Err(BridgeError::AmountExceedsTransactionLimit);
        }

        let now = env.ledger().timestamp();
        let today = day_of(now);
        let new_daily_total = storage::get_daily_total(&env, &agent_id, today)
            .checked_add(usdc_amount)
            .ok_or(BridgeError::AmountExceedsDailyLimit)?;
        if new_daily_total > state.max_per_day {
            return Err(BridgeError::AmountExceedsDailyLimit);
        }

        let token = token::Client::new(&env, &state.usdc_token);
        if token.balance(&state.reserve_account) < usdc_amount {
            return Err(BridgeError::InsufficientReserve);
        }
        // Requires the reserve account to authorize the transfer. The Stellar
        // Service collects that authorization from the 2-of-3 reserve signers.
        token.transfer(&state.reserve_account, &agent.stellar_address, &usdc_amount);

        state.total_converted += usdc_amount;
        storage::set_state(&env, &state);

        agent.lifetime_converted += usdc_amount;
        agent.today_converted = new_daily_total;
        agent.last_conversion_ts = now;
        agent.conversion_count += 1;
        storage::set_agent(&env, &agent_id, &agent);
        storage::set_daily_total(&env, &agent_id, today, new_daily_total);

        let record = ConversionRecord {
            conversion_id: conversion_id.clone(),
            agent_id: agent_id.clone(),
            stellar_address: agent.stellar_address.clone(),
            kes_amount,
            usdc_amount,
            timestamp: now,
            ledger_sequence: env.ledger().sequence(),
            reversed: false,
        };
        storage::set_conversion(&env, &conversion_id, &record);

        events::emit_conversion_completed(
            &env,
            conversion_id,
            agent_id,
            agent.stellar_address,
            kes_amount,
            usdc_amount,
        );
        Ok(record)
    }

    /// Reverse a conversion (error recovery). Admin only.
    /// Marks the conversion record as reversed and removes its amount from the
    /// agent's lifetime and daily totals, so the agent's limits reflect net
    /// credits. USDC is not moved on-chain here; see README.
    /// `reason` is accepted for the operator's audit trail but is not stored,
    /// because no event or record field is defined for it in the TRD.
    pub fn reverse_convert(
        env: Env,
        conversion_id: String,
        reason: String,
    ) -> Result<(), BridgeError> {
        let _ = reason;
        let state = load_state(&env)?;
        state.admin.require_auth();

        let mut record = storage::get_conversion(&env, &conversion_id)
            .ok_or(BridgeError::ConversionNotFound)?;
        if record.reversed {
            return Err(BridgeError::AlreadyReversed);
        }
        record.reversed = true;
        storage::set_conversion(&env, &conversion_id, &record);

        // `total_converted` is intentionally not decreased: it mirrors the sum of
        // ConversionCompleted events, which cannot be retracted.
        let mut agent = storage::get_agent(&env, &record.agent_id)
            .ok_or(BridgeError::AgentNotRegistered)?;
        agent.lifetime_converted -= record.usdc_amount;
        storage::set_agent(&env, &record.agent_id, &agent);

        let day = day_of(record.timestamp);
        let daily = storage::get_daily_total(&env, &record.agent_id, day) - record.usdc_amount;
        storage::set_daily_total(&env, &record.agent_id, day, daily);
        if day == day_of(env.ledger().timestamp()) {
            agent.today_converted = daily;
            storage::set_agent(&env, &record.agent_id, &agent);
        }
        Ok(())
    }

    /// Pause the bridge. Admin only. Rejects all new conversions until unpaused.
    pub fn pause(env: Env) -> Result<(), BridgeError> {
        let mut state = load_state(&env)?;
        state.admin.require_auth();
        state.paused = true;
        storage::set_state(&env, &state);
        events::emit_bridge_paused(&env, state.admin);
        Ok(())
    }

    /// Unpause the bridge. Admin only.
    pub fn unpause(env: Env) -> Result<(), BridgeError> {
        let mut state = load_state(&env)?;
        state.admin.require_auth();
        state.paused = false;
        storage::set_state(&env, &state);
        events::emit_bridge_unpaused(&env, state.admin);
        Ok(())
    }

    /// Update per-transaction or per-day limits. Admin only.
    pub fn set_limits(
        env: Env,
        max_per_tx_usdc: Option<i128>,
        max_per_day_usdc: Option<i128>,
    ) -> Result<(), BridgeError> {
        let mut state = load_state(&env)?;
        state.admin.require_auth();

        if let Some(max_tx) = max_per_tx_usdc {
            if max_tx <= 0 {
                return Err(BridgeError::InvalidAmount);
            }
            state.max_per_tx = max_tx;
        }
        if let Some(max_day) = max_per_day_usdc {
            if max_day <= 0 {
                return Err(BridgeError::InvalidAmount);
            }
            state.max_per_day = max_day;
        }
        storage::set_state(&env, &state);
        Ok(())
    }

    /// Transfer admin role to a new address. Current admin must authorize.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), BridgeError> {
        let mut state = load_state(&env)?;
        state.admin.require_auth();
        state.admin = new_admin;
        storage::set_state(&env, &state);
        Ok(())
    }

    /// Read current bridge state (paused, limits, total_converted).
    pub fn get_state(env: Env) -> Result<BridgeState, BridgeError> {
        load_state(&env)
    }

    /// Read an agent's current state (balances, conversion count).
    /// `today_converted` is recomputed from the daily total, so it is 0 on a new day.
    pub fn get_agent(env: Env, agent_id: String) -> Result<AgentAccount, BridgeError> {
        let mut agent =
            storage::get_agent(&env, &agent_id).ok_or(BridgeError::AgentNotRegistered)?;
        agent.today_converted =
            storage::get_daily_total(&env, &agent_id, day_of(env.ledger().timestamp()));
        Ok(agent)
    }

    /// Read a specific conversion record by ID.
    pub fn get_conversion(env: Env, conversion_id: String) -> Result<ConversionRecord, BridgeError> {
        storage::get_conversion(&env, &conversion_id).ok_or(BridgeError::ConversionNotFound)
    }

    /// Read cumulative USDC credited (on-chain metric for SCF milestone).
    /// Returns 0 before initialization.
    pub fn get_total_converted(env: Env) -> i128 {
        storage::get_state(&env).map_or(0, |state| state.total_converted)
    }

    /// Read USDC credited today for an agent (for real-time limit checking off-chain).
    /// Returns 0 for an unknown agent.
    pub fn get_agent_daily_total(env: Env, agent_id: String) -> i128 {
        storage::get_daily_total(&env, &agent_id, day_of(env.ledger().timestamp()))
    }
}
