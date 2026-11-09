use soroban_sdk::{contracterror, contracttype, Address, String};

/// Top-level bridge configuration and state.
#[contracttype]
pub struct BridgeState {
    /// Contract admin (PesaKit operations key).
    pub admin: Address,
    /// Address of the USDC token contract on Stellar mainnet.
    pub usdc_token: Address,
    /// Bridge reserve account (multisig, holds USDC to be distributed).
    pub reserve_account: Address,
    /// Maximum USDC per single conversion (in stroops: 100 USDC = 1_000_000_000).
    pub max_per_tx: i128,
    /// Maximum USDC per agent per calendar day (in stroops).
    pub max_per_day: i128,
    /// Whether the bridge is paused. No conversions accepted when true.
    pub paused: bool,
    /// Total USDC credited since deployment (cumulative on-chain metric).
    pub total_converted: i128,
}

/// Per-agent state stored on-chain.
#[contracttype]
pub struct AgentAccount {
    /// The agent's Stellar account ID (recipient of USDC).
    pub stellar_address: Address,
    /// Total USDC credited to this agent since deployment.
    pub lifetime_converted: i128,
    /// USDC credited today (resets at UTC midnight).
    pub today_converted: i128,
    /// Ledger timestamp of the last conversion (seconds since Unix epoch).
    pub last_conversion_ts: u64,
    /// Sequential conversion counter for this agent.
    pub conversion_count: u64,
}

/// Immutable record of a single conversion, keyed by conversion_id.
#[contracttype]
pub struct ConversionRecord {
    /// Unique, generated off-chain, max 64 chars.
    pub conversion_id: String,
    /// PesaKit agent ID, max 32 chars.
    pub agent_id: String,
    pub stellar_address: Address,
    /// KES deposited (integer, no decimal).
    pub kes_amount: i128,
    /// USDC credited (stroops).
    pub usdc_amount: i128,
    pub timestamp: u64,
    pub ledger_sequence: u32,
}

/// Contract error codes.
///
/// Declared with `#[contracterror]` (not `#[contracttype]`) because it is
/// returned in `Result<_, BridgeError>` from contract functions.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BridgeError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    BridgePaused = 4,
    AmountExceedsTransactionLimit = 5,
    AmountExceedsDailyLimit = 6,
    /// Zero or negative amount.
    InvalidAmount = 7,
    AgentNotRegistered = 8,
    DuplicateConversionId = 9,
    InsufficientReserve = 10,
    InvalidAddress = 11,
}
