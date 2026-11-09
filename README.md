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
| `reverse_convert` | Reverse a conversion for error recovery | Admin |
| `pause` / `unpause` | Halt or resume new conversions | Admin |
| `set_limits` | Update per-transaction or daily limits | Admin |
| `transfer_admin` | Hand the admin role to a new address | Admin |
| `get_state` | Read bridge state, limits and total converted | Read-only |
| `get_agent` | Read an agent's account state | Read-only |
| `get_conversion` | Read a conversion record by ID | Read-only |
| `get_total_converted` | Read cumulative USDC credited | Read-only |
| `get_agent_daily_total` | Read USDC credited today for an agent | Read-only |

## On-chain Metric

Every successful conversion emits a `ConversionCompleted` event, making cumulative USDC independently verifiable on Stellar Horizon or stellar.expert without access to PesaKit's internal systems. The metric is the sum of `usdc_amount` across all `ConversionCompleted` events emitted by this contract.

## Transaction Limits

- Maximum per transaction: $100 USDC
- Maximum per agent per day: KSh 100,000 (~$750 USDC)
- Bridge reserve: 2-of-3 multisig account, all keys held by PesaKit

## Build

```bash
cargo check --target wasm32-unknown-unknown
```

Note that this is a skeleton; `todo!()` stubs replace full implementations.

## Testnet Deployment

A testnet deployment will follow in Tranche 1 (target: January 2027).

Testnet contract ID: _not yet deployed_

## Integration Partners

- [SoroSwap](https://soroswap.finance): Soroban DEX; agents supply USDC as LP to earn yield
- [DeFindex](https://defindex.io): managed yield strategies on Stellar
- [Aquarius](https://aqua.network): liquidity aggregation layer routing through SoroSwap

## SCF Grant

This contract is part of the SCF #46 Build Award application (Integration Track).

## License

Apache-2.0. See [LICENSE](LICENSE).
