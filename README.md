# PesaKit Stellar Bridge

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![CI](https://github.com/pesakitdev/stellar/actions/workflows/check.yml/badge.svg)](https://github.com/pesakitdev/stellar/actions/workflows/check.yml)

## Overview

PesaKit connects Kenya's 70,000+ mobile-money agents to the Stellar/Soroban ecosystem. This contract is the on-chain component of a KES→USDC bridge that lets agents convert mobile-money float into USDC, then deploy it into on-chain yield via SoroSwap (LP deposits) or DeFindex (managed strategies).

## Architecture

- **Mobile money adapters (existing):** M-Pesa, Airtel Money and Telkom adapters confirm each KES payment.
- **Bridge Controller (off-chain):** checks KYC and sanctions, enforces the per-transaction and daily limits, and triggers the conversion.
- **Stellar Service (off-chain):** submits Soroban calls to this contract, manages agent accounts and USDC trustlines, and monitors the reserve.
- **Soroban bridge contract (this repository):** records conversions, enforces limits on-chain, and emits the events used for the on-chain metric.

Full interactive diagram: <https://pesakit.ai/stellar/PesaKit-Stellar-Architecture.html>

## Contract Interface Summary

| Function | Purpose | Caller |
|---|---|---|
| `initialize` | One-time setup: admin, USDC token, reserve account, limits | Deployer (once) |
| `register_agent` | Register an agent's Stellar address | Admin |
| `convert` | Record a KES→USDC conversion and credit USDC | Stellar Service (admin-authorized) |
| `reverse_convert` | Reverse a conversion for error recovery, with a short reason code | Admin |
| `return_usdc` | Move USDC from an agent's wallet back to the reserve and record it; PesaKit then pays the KES off-chain | Admin and the agent's wallet |
| `pause` / `unpause` | Halt or resume new conversions | Admin |
| `set_limits` | Update per-transaction or daily limits | Admin |
| `transfer_admin` | Hand the admin role to a new address | Admin |
| `get_state` | Read bridge state, limits and total converted | Read-only |
| `get_agent` | Read an agent's account state | Read-only |
| `get_conversion` | Read a conversion record by ID | Read-only |
| `get_total_converted` | Read cumulative USDC credited | Read-only |
| `get_agent_daily_total` | Read USDC credited today for an agent | Read-only |
| `get_return` | Read a return record by ID | Read-only |
| `get_total_returned` | Read cumulative USDC returned to the reserve | Read-only |
| `get_agent_daily_returned` | Read USDC an agent returned today | Read-only |

## On-chain Metric

Every successful conversion emits a `ConversionCompleted` event, making cumulative USDC independently verifiable on Stellar Horizon or stellar.expert without access to PesaKit's internal systems. The metric is the sum of `usdc_amount` across all `ConversionCompleted` events emitted by this contract.

A reversal emits a `ConversionReversed` event with the conversion ID, agent ID, agent address, USDC amount and a reason code of at most 32 characters (for example `duplicate_payment`). The original `ConversionCompleted` event stays on the ledger, so the gross metric is unchanged and the net amount is the completed total minus the reversed total. Reversal frees the agent's limits and flags the record; it does not move USDC. The reason is public, so it is a code and never personal data.

## Transaction Limits

- Maximum per transaction: $100 USDC
- Maximum per agent per day: KSh 100,000 (about $750 at the current rate), enforced by the Bridge Controller. The contract enforces a USDC daily cap as a backstop. Daily totals reset at midnight Nairobi time (21:00 UTC). Returns to the reserve have their own daily total, so each direction has its own limit.
- Bridge reserve: 2-of-3 multisig account, all keys held by PesaKit

## Build

```bash
cargo check --target wasm32v1-none
cargo test --manifest-path contracts/bridge/Cargo.toml
```

Build the deployable contract with the Stellar CLI, which sets the build markers that soroban-sdk requires: `stellar contract build`. Use `wasm32v1-none`, not `wasm32-unknown-unknown`: the SDK refuses the latter on Rust 1.82 and later.

The contract implements the full interface in `contracts/bridge/src/lib.rs`. The unit tests in `contracts/bridge/src/test.rs` cover the limits, pause, duplicate-ID checks, reversals, and admin functions.

Conversions move USDC from the reserve account to the agent with a token transfer. The reserve account must authorize that transfer. The Stellar Service collects this authorization from the 2-of-3 reserve signers.

## Testnet Deployment

The bridge contract is deployed on Stellar testnet. The current version is built with soroban-sdk 28 and emits `ConversionReversed` events.

- Testnet contract ID: `CB4XNHY22TXAI2OUMUGHIK2I3S7BWCTJNV6ERFWDLJBKJMZRTJDVKZTE`
- Test USDC (testnet only, no value): `CATPOI2F4AXIPYIMLA2Z6FNEXXHDLT3RKVKTGG3FGLASXXBOIS5MSP3J`
- Network: Stellar testnet (`Test SDF Network ; September 2015`)
- Wasm hash: `8ec12e421a7b11fc61a044edddac1ab59c79044deaec0fd77dd40a2ff276c3c1`, built by the release workflow at tag `v0.2.3`. [Stellar Expert](https://stellar.expert/explorer/testnet/contract/CB4XNHY22TXAI2OUMUGHIK2I3S7BWCTJNV6ERFWDLJBKJMZRTJDVKZTE) links this contract to the source.

### Verifying the testnet contract

Anyone can check that the deployed contract matches this source:

1. Open the [v0.2.3 release](https://github.com/pesakitdev/stellar/releases) and download `pesakit-bridge_v0.1.0.wasm`.
2. Hash the file with SHA-256. The hash should be `8ec12e421a7b11fc61a044edddac1ab59c79044deaec0fd77dd40a2ff276c3c1`, the wasm hash of contract `CB4XNHY22TXAI2OUMUGHIK2I3S7BWCTJNV6ERFWDLJBKJMZRTJDVKZTE`.
3. To rebuild it yourself, use the release workflow (`.github/workflows/release.yml`), which builds on GitHub with pinned tooling. A local `stellar contract build` gives a different hash, so compare against the release asset.

### Releasing a new contract version

1. Merge the change to `main` and make sure CI passes.
2. Create a version tag, for example `v0.2.4`, and push it. The release workflow builds the contract, publishes a GitHub release with the wasm and its SHA-256 hash, and sends the hash to Stellar Expert.
3. Check that the release exists and note its wasm hash.
4. Deploy the release wasm (not a local build) with `stellar contract deploy`, then run `initialize` and `register_agent`.
5. Add the new contract ID and wasm hash to this README, and update the website.

Earlier testnet builds, kept for reference: `CDOO6HK7UJYBCDKC6WKIWI4AFEM2GGZ6I7R5TYG2YLANL7EYUX7PSOHF` (soroban-sdk 28, built locally, not linked on the explorer), `CC7IHCWLG25EORHZLWEWSTQGXKMUO2TK2TDVCMFNCB6ISCDWGWE75Q6P` (soroban-sdk 21, with the reversal event) and `CBHBAUR2ITTAU5GV7XWZFDI4TW3JVOJC27UDLXBGNBK6EAYPO62747MC` (without it). Mainnet deployment is scheduled for the first week of April 2027.

## Integration Partners

- [SoroSwap](https://soroswap.finance): Soroban DEX; agents supply USDC as LP to earn yield. SoroSwap is used only for LP and yield deposits, never for cross-border sends.
- [DeFindex](https://defindex.io): managed yield strategies on Stellar
- [Aquarius](https://aqua.network): liquidity aggregation layer routing through SoroSwap
- MoneyGram (future phase): cash-in and cash-out at agent locations for diaspora remittances. It is not part of this contract or the current bridge.

## SCF Grant

This contract is part of the SCF #46 Build Award application (Integration Track).

## License

Apache-2.0. See [LICENSE](LICENSE).
