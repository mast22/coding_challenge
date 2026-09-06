## Why

The crate is a scaffold: empty `src/` modules, a placeholder integration test, and a sample CSV. The coding challenge is to process a chronological stream of payment events and emit every client's final account snapshot (`available`, `held`, `total`, `locked`) with four-decimal money math and bank-like ignore rules for partner errors.

## What Changes

- Add a pure sequential `Engine` that applies typed events (`deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback`) to per-client accounts and a deposit log.
- Stream-read input CSV rows into events without loading the whole file; skip malformed rows and unknown types; write account snapshots as CSV to stdout.
- Add a CLI binary: `cargo run -- transactions.csv` (no clap). Missing or unreadable files fail the process; partner-semantic mistakes are silent ignores.
- Cover the brief's scenarios with engine unit tests and `tests/` integration fixtures, including the sample CSV already at `tests/test_data/sample.csv`.

## Capabilities

### New Capabilities

- `transaction-engine`: Sequential reducer for payment events, account snapshots, deposit lifecycle (settled / disputed / charged-back), lock, and ledger invariants.
- `csv-io`: Streaming CSV input of events and CSV output of account snapshots, including whitespace and decimal-precision tolerance.
- `cli`: Binary entry that takes one CSV path, streams events through the engine, and writes snapshots to stdout.

### Modified Capabilities

- None. `openspec/specs/` is empty.

## Impact

- Fills the existing layout: `src/lib.rs`, `src/main.rs`, `src/types/`, `src/engine/`, `src/io/`, `src/error.rs`, `tests/`.
- Uses crates already in `Cargo.toml`: `rust_decimal` (serde string), `serde`, `csv`, `anyhow`, `thiserror`. No clap. CLI args via `std::env`.
- Public library API is for tests: `Engine`, event/account types, CSV consumer/producer.
- Output row order is unspecified; money fields are four decimal places.
- Partner errors never abort the stream; only I/O / missing-path failures are process errors.
