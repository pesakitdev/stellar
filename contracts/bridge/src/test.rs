#![cfg(test)]

extern crate std;
use std::boxed::Box;

use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env, IntoVal, String, Symbol,
};

use crate::types::BridgeError;
use crate::{PesaKitBridge, PesaKitBridgeClient};

/// 1 USDC in stroops (7 decimal places).
const USDC: i128 = 10_000_000;
const MAX_PER_TX: i128 = 100 * USDC;
const MAX_PER_DAY: i128 = 150 * USDC;
const DAY: u64 = 86_400;

struct Fixture {
    env: Env,
    client: PesaKitBridgeClient<'static>,
    admin: Address,
    usdc: Address,
    reserve: Address,
    agent_wallet: Address,
}

fn setup() -> Fixture {
    // Leaked so the client can borrow it for 'static without a self-referential struct.
    let env: &'static Env = Box::leak(Box::new(Env::default()));
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 10 * DAY);

    let contract_id = env.register_contract(None, PesaKitBridge);
    let client = PesaKitBridgeClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let usdc = env.register_stellar_asset_contract_v2(token_admin).address();
    let reserve = Address::generate(env);
    let agent_wallet = Address::generate(env);

    StellarAssetClient::new(env, &usdc).mint(&reserve, &(10_000 * USDC));

    client.initialize(&admin, &usdc, &reserve, &MAX_PER_TX, &MAX_PER_DAY);
    client.register_agent(&String::from_str(env, "agent-1"), &agent_wallet);

    Fixture {
        env: env.clone(),
        client,
        admin,
        usdc,
        reserve,
        agent_wallet,
    }
}

/// Builds a soroban String inside the fixture's environment.
fn sid(f: &Fixture, s: &str) -> String {
    String::from_str(&f.env, s)
}

fn token_balance(f: &Fixture, who: &Address) -> i128 {
    TokenClient::new(&f.env, &f.usdc).balance(who)
}

#[test]
fn initialize_stores_state() {
    let f = setup();
    let state = f.client.get_state();
    assert_eq!(state.admin, f.admin);
    assert_eq!(state.usdc_token, f.usdc);
    assert_eq!(state.reserve_account, f.reserve);
    assert_eq!(state.max_per_tx, MAX_PER_TX);
    assert_eq!(state.max_per_day, MAX_PER_DAY);
    assert!(!state.paused);
    assert_eq!(state.total_converted, 0);
}

#[test]
fn initialize_twice_fails() {
    let f = setup();
    let result = f
        .client
        .try_initialize(&f.admin, &f.usdc, &f.reserve, &MAX_PER_TX, &MAX_PER_DAY);
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::AlreadyInitialized);
}

#[test]
fn initialize_rejects_non_positive_limits() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PesaKitBridge);
    let client = PesaKitBridgeClient::new(&env, &contract_id);
    let a = Address::generate(&env);
    let result = client.try_initialize(&a, &a, &a, &0, &MAX_PER_DAY);
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::InvalidAmount);
}

#[test]
fn calls_before_initialize_fail() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PesaKitBridge);
    let client = PesaKitBridgeClient::new(&env, &contract_id);
    let result = client.try_get_state();
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::NotInitialized);
    assert_eq!(client.get_total_converted(), 0);
}

#[test]
fn convert_transfers_usdc_and_updates_metric() {
    let f = setup();
    let record = f.client.convert(
        &sid(&f, "conv-1"),
        &sid(&f, "agent-1"),
        &1_300_00, // KES 13,000 as an integer
        &(50 * USDC),
    );

    assert_eq!(record.usdc_amount, 50 * USDC);
    assert!(!record.reversed);
    assert_eq!(token_balance(&f, &f.agent_wallet), 50 * USDC);
    assert_eq!(token_balance(&f, &f.reserve), 10_000 * USDC - 50 * USDC);
    assert_eq!(f.client.get_total_converted(), 50 * USDC);
    assert_eq!(f.client.get_agent_daily_total(&sid(&f, "agent-1")), 50 * USDC);

    let agent = f.client.get_agent(&sid(&f, "agent-1"));
    assert_eq!(agent.lifetime_converted, 50 * USDC);
    assert_eq!(agent.conversion_count, 1);
    assert_eq!(agent.today_converted, 50 * USDC);

    let stored = f.client.get_conversion(&sid(&f, "conv-1"));
    assert_eq!(stored.kes_amount, 1_300_00);
}

#[test]
fn convert_rejects_amount_over_per_tx_limit() {
    let f = setup();
    let result = f.client.try_convert(
        &sid(&f, "conv-1"),
        &sid(&f, "agent-1"),
        &1,
        &(MAX_PER_TX + 1),
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        BridgeError::AmountExceedsTransactionLimit
    );
}

#[test]
fn convert_enforces_daily_limit_and_resets_next_day() {
    let f = setup();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &(90 * USDC));

    // 90 + 70 = 160 > 150 per day.
    let over = f
        .client
        .try_convert(&sid(&f, "conv-2"), &sid(&f, "agent-1"), &1, &(70 * USDC));
    assert_eq!(
        over.unwrap_err().unwrap(),
        BridgeError::AmountExceedsDailyLimit
    );

    // Limits are per agent per day, so the next UTC day starts again.
    f.env.ledger().with_mut(|l| l.timestamp = 11 * DAY + 5);
    f.client
        .convert(&sid(&f, "conv-3"), &sid(&f, "agent-1"), &1, &(70 * USDC));
    assert_eq!(f.client.get_agent_daily_total(&sid(&f, "agent-1")), 70 * USDC);
    assert_eq!(f.client.get_total_converted(), 160 * USDC);
}

#[test]
fn convert_rejects_non_positive_amounts() {
    let f = setup();
    let zero = f
        .client
        .try_convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &0);
    assert_eq!(zero.unwrap_err().unwrap(), BridgeError::InvalidAmount);

    let negative_kes = f
        .client
        .try_convert(&sid(&f, "conv-2"), &sid(&f, "agent-1"), &-1, &USDC);
    assert_eq!(negative_kes.unwrap_err().unwrap(), BridgeError::InvalidAmount);
}

#[test]
fn convert_rejects_duplicate_conversion_id() {
    let f = setup();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);
    let dup = f
        .client
        .try_convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);
    assert_eq!(dup.unwrap_err().unwrap(), BridgeError::DuplicateConversionId);
}

#[test]
fn convert_rejects_unregistered_agent() {
    let f = setup();
    let result = f
        .client
        .try_convert(&sid(&f, "conv-1"), &sid(&f, "ghost"), &1, &USDC);
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::AgentNotRegistered);
}

#[test]
fn convert_rejects_when_reserve_is_short() {
    let f = setup();
    // Drain the reserve so it cannot fund even 1 USDC.
    let sink = Address::generate(&f.env);
    TokenClient::new(&f.env, &f.usdc).transfer(&f.reserve, &sink, &(10_000 * USDC));

    let result = f
        .client
        .try_convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::InsufficientReserve);
}

#[test]
fn pause_blocks_conversions_until_unpaused() {
    let f = setup();
    f.client.pause();
    assert!(f.client.get_state().paused);

    let paused = f
        .client
        .try_convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);
    assert_eq!(paused.unwrap_err().unwrap(), BridgeError::BridgePaused);

    f.client.unpause();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);
    assert_eq!(f.client.get_total_converted(), USDC);
}

#[test]
fn reverse_marks_record_and_reduces_limits() {
    let f = setup();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &(80 * USDC));

    f.client
        .reverse_convert(&sid(&f, "conv-1"), &sid(&f, "duplicate_payment"));

    assert!(f.client.get_conversion(&sid(&f, "conv-1")).reversed);
    assert_eq!(f.client.get_agent_daily_total(&sid(&f, "agent-1")), 0);
    assert_eq!(
        f.client.get_agent(&sid(&f, "agent-1")).lifetime_converted,
        0
    );
    // Events cannot be retracted, so the cumulative metric keeps the gross amount.
    assert_eq!(f.client.get_total_converted(), 80 * USDC);

    let again = f
        .client
        .try_reverse_convert(&sid(&f, "conv-1"), &sid(&f, "again"));
    assert_eq!(again.unwrap_err().unwrap(), BridgeError::AlreadyReversed);
}

#[test]
fn reverse_emits_conversion_reversed_event() {
    let f = setup();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &(80 * USDC));
    f.client
        .reverse_convert(&sid(&f, "conv-1"), &sid(&f, "duplicate_payment"));

    let events = f.env.events().all();
    let last = events.slice(events.len() - 1..);
    assert_eq!(
        last,
        vec![
            &f.env,
            (
                f.client.address.clone(),
                (Symbol::new(&f.env, "ConversionReversed"), f.agent_wallet.clone()).into_val(&f.env),
                (
                    sid(&f, "conv-1"),
                    sid(&f, "agent-1"),
                    f.agent_wallet.clone(),
                    80 * USDC,
                    sid(&f, "duplicate_payment"),
                )
                    .into_val(&f.env),
            ),
        ]
    );
}

#[test]
fn reverse_rejects_empty_or_long_reasons() {
    let f = setup();
    f.client
        .convert(&sid(&f, "conv-1"), &sid(&f, "agent-1"), &1, &USDC);

    let empty = f.client.try_reverse_convert(&sid(&f, "conv-1"), &sid(&f, ""));
    assert_eq!(empty.unwrap_err().unwrap(), BridgeError::InvalidId);

    let long = f.client.try_reverse_convert(
        &sid(&f, "conv-1"),
        &sid(&f, "a_reason_code_that_is_over_32_chars"),
    );
    assert_eq!(long.unwrap_err().unwrap(), BridgeError::InvalidId);

    // A rejected reversal changes nothing.
    assert!(!f.client.get_conversion(&sid(&f, "conv-1")).reversed);
}

#[test]
fn reverse_unknown_conversion_fails() {
    let f = setup();
    let result = f
        .client
        .try_reverse_convert(&sid(&f, "nope"), &sid(&f, "reason"));
    assert_eq!(
        result.unwrap_err().unwrap(),
        BridgeError::ConversionNotFound
    );
}

#[test]
fn duplicate_agent_registration_fails() {
    let f = setup();
    let other = Address::generate(&f.env);
    let result = f.client.try_register_agent(&sid(&f, "agent-1"), &other);
    assert_eq!(
        result.unwrap_err().unwrap(),
        BridgeError::AgentAlreadyRegistered
    );
}

#[test]
fn oversized_ids_are_rejected() {
    let f = setup();
    let long_agent = "a".repeat(33);
    let result = f
        .client
        .try_register_agent(&sid(&f, &long_agent), &f.agent_wallet);
    assert_eq!(result.unwrap_err().unwrap(), BridgeError::InvalidId);
}

#[test]
fn set_limits_validates_and_applies() {
    let f = setup();
    let bad = f.client.try_set_limits(&Some(0), &None);
    assert_eq!(bad.unwrap_err().unwrap(), BridgeError::InvalidAmount);

    f.client.set_limits(&Some(20 * USDC), &None);
    let state = f.client.get_state();
    assert_eq!(state.max_per_tx, 20 * USDC);
    assert_eq!(state.max_per_day, MAX_PER_DAY);
}

#[test]
fn transfer_admin_moves_control() {
    let f = setup();
    let new_admin = Address::generate(&f.env);
    f.client.transfer_admin(&new_admin);
    assert_eq!(f.client.get_state().admin, new_admin);
}
