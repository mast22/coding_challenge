## 1. Types and module wiring

- [x] 1.1 Fill `src/types/mod.rs` with `ClientId`, `TxId`, `Amount` (`Decimal` scale 4), `Event`, `Account` (derived `total()`), `DepositRecord`, and `DepositState`; verify `cargo check` succeeds and no `f32`/`f64` money fields exist
- [x] 1.2 Fill `src/error.rs` with a `thiserror` row/parse error type for skippable CSV rows; verify the type implements `std::error::Error` and `Display`
- [x] 1.3 Wire `src/lib.rs`, `src/engine/mod.rs`, and `src/io/mod.rs` so `Engine`, `Event`, `Account`, `CSVConsumer`, and `StdoutProducer` are reachable; replace `src/io/writer.rs` with `src/io/stdout.rs`; verify `cargo check` still succeeds

## 2. Engine deposits and withdrawals

- [x] 2.1 Implement `Engine::new`, `apply`, `accounts`, and the deposit/withdrawal handlers with account creation, `seen_txs`, and post-mutation `debug_assert` invariants; verify a unit test that a single deposit of 1.0 yields available 1.0000, held 0, total 1.0000, locked false
- [x] 2.2 Add unit tests for two deposits summing to 3.0000, successful withdraw 2.0 then 1.5 → 0.5000, insufficient withdraw 2.0 then 3.0 unchanged, exact withdraw to zeros, and 1.2345 then 0.0001 → 1.2344; verify `cargo test --lib` passes those cases
- [x] 2.3 Add unit tests for first-event withdrawal creating a zeros account, interleaved brief-sample events matching client 1 = 1.5 and client 2 = 2.0, zero/negative amounts ignored, duplicate deposit tx ignored, and four-decimal 1.0001 + 2.0002 = 3.0003; verify `cargo test --lib` passes

## 3. Dispute, resolve, chargeback, and lock

- [x] 3.1 Implement dispute/resolve/chargeback handlers (client match, Settled→Disputed→Settled/ChargedBack, available-sufficient hold); verify unit tests: dispute moves 1.0 to held, resolve restores, chargeback yields available 0 held 0 total 0 locked true
- [x] 3.2 Add unit tests for dispute one of two deposits (available 2 held 1 total 3), double dispute, re-dispute after resolve, cannot re-dispute after chargeback, resolve then chargeback ignored, chargeback then resolve ignored, withdraw while held fails, withdraw after resolve 10-4=6; verify `cargo test --lib` passes
- [x] 3.3 Add unit tests that after chargeback, further deposit/withdrawal/dispute/resolve/chargeback on that client are ignored and locked stays true; verify `cargo test --lib` passes

## 4. Partner ignores and invariants

- [x] 4.1 Add unit tests for unknown tx on dispute/resolve/chargeback, resolve/chargeback on undisputed tx, dispute of a withdrawal tx, wrong-client dispute, dispute after partial spend (deposit 10 withdraw 6 dispute 10 ignored), and stream order with tx 5 before tx 1; verify `cargo test --lib` passes
- [x] 4.2 Add an invariant helper used after every `apply` in engine tests (available >= 0, held >= 0, total = available + held, locked sticky, held equals sum of Disputed deposits for that client); verify the helper is called from the existing engine tests and `cargo test --lib` still passes

## 5. CSV I/O

- [x] 5.1 Implement `CSVConsumer::new` and `iter()` with `csv::ReaderBuilder` trim-all, streaming rows, and mapping to `Event`; verify a unit or integration test that `tests/test_data/sample.csv` yields five events in file order
- [x] 5.2 Add tests that whitespace around fields and amounts `1` / `1.0` / `1.0000` parse, and that unknown type, missing deposit amount, and unparsable numbers return skippable row errors without aborting later rows; verify those tests pass
- [x] 5.3 Implement `StdoutProducer::new` and `write` serializing `client,available,held,total,locked` at four-decimal precision; verify a test that writing the brief-sample snapshots produces those two rows in any order

## 6. CLI

- [x] 6.1 Implement `run(path)` in `src/main.rs`: stream `CSVConsumer`, skip row errors, `engine.apply`, then `StdoutProducer` to stdout; `main` reads one path from `std::env::args` with no clap; verify `cargo run -- tests/test_data/sample.csv` prints two account rows matching the brief (order irrelevant) and exits 0
- [x] 6.2 Verify missing path and unreadable path are process errors (non-zero exit), and that a fixture mixing valid and malformed rows still writes snapshots for the valid clients

## 7. Integration fixtures

- [x] 7.1 Replace `tests/integration_tests.rs` placeholder with CSV-fixture tests covering the brief sample, dispute/resolve/chargeback happy paths, lock, and a malformed-row skip; verify `cargo test` passes the full suite
