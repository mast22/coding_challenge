## Context

See proposal.md for motivation. The crate already has the target layout as empty files, `Cargo.toml` with `rust_decimal` (serde string), `serde`, `csv`, and `anyhow`, a stub `main` that prints `Hello world!`, a placeholder `tests/integration_tests.rs`, and `tests/test_data/sample.csv` matching the brief sample. `openspec/specs/` is empty. I/O must stay at the edge; the engine is a pure sequential reducer. Datasets may be huge: stream events, do not collect the full list.

## Goals / Non-Goals

**Goals:**

- Keep the engine free of CSV, stdout, and CLI concerns.
- Stream one event at a time; bound memory to accounts plus a single tx map (disputable deposits and consumed withdrawal ids).
- Use decimal scale-4 money; never `f32`/`f64` for mutation.
- Treat partner mistakes as ignore; treat missing/unreadable files as process errors.

**Non-Goals:**

- Concurrent apply, persistence, networking, or a public HTTP API.
- Reordering, replay, or idempotent ingestion beyond the documented ignore rules.
- Clap or any CLI framework.
- Inventing balances or emitting clients that never appeared in the stream.

## Decisions

### 1. Module layout

Reuse the scaffold. Rename the unused writer stub to match the requested stdout type.

```
src/lib.rs                 private modules; re-export Engine, Event, Account, CSVConsumer, StdoutProducer
src/main.rs                main + run(path)
src/types/mod.rs           ClientId, TxId, Amount, Event, Account (crate-private fields, public getters)
src/engine/mod.rs          pub use engine::Engine
src/engine/engine.rs       Engine state machine; crate-private DepositRecord / DepositState / TxRecord
src/io/mod.rs              pub mod csv; pub mod stdout
src/io/csv.rs              CSVConsumer
src/io/stdout.rs           StdoutProducer  (replace empty src/io/writer.rs)
```

`lib.rs` exists so integration tests can call `Engine` without spawning the binary for every case. The binary only wires I/O to `apply`. `DepositRecord` is not on the public crate surface.

Alternative: a single `src/engine.rs` file. Rejected because the brief asked for an `engine` directory and the scaffold already has it.

### 2. CLI args vs CSV parsing

Section 9 says "parsing csv use only std lib (do not use clap)" while section 10 and `Cargo.toml` already specify `csv` + `serde`. Decision: **no clap**; `std::env::args` for the path. **CSV uses the `csv` and `serde` crates** already installed. Trim whitespace with `csv::ReaderBuilder::new().trim(csv::Trim::All).flexible(true)`.

Alternative: hand-rolled split-on-comma. Rejected; the challenge crate already depends on `csv`/`serde`, and flexible quoting/whitespace is their job.

### 3. Types

```
ClientId = u16
TxId     = u32
Amount   = rust_decimal::Decimal   // rescale(4) on parse; never f32/f64
```

`Event` is an enum with amount on `Deposit`/`Withdrawal` only:

```
enum Event {
    Deposit { client: ClientId, tx: TxId, amount: Amount },
    Withdrawal { client: ClientId, tx: TxId, amount: Amount },
    Dispute { client: ClientId, tx: TxId },
    Resolve { client: ClientId, tx: TxId },
    Chargeback { client: ClientId, tx: TxId },
}
```

`Account` stores `available`, `held`, `locked` as crate-private fields with public read-only getters. `total()` is `available + held` (derived, not stored). Engine-internal `DepositRecord` stores `client`, `amount`, `state: Settled | Disputed | ChargedBack` (tx id is the map key).

CSV input uses a private row struct (`type`/`client`/`tx`/`amount`) then maps to `Event`. Unknown `type` or missing amount on deposit/withdrawal is dropped by the reader, not modeled as a typed error a caller inspects.

### 4. Engine state and apply pipeline

```
enum TxRecord {
    Deposit(DepositRecord),  // successful, disputable
    Consumed,                // withdrawal that took the id (success or insufficient)
}

struct Engine {
    accounts: HashMap<ClientId, Account>,
    txs: HashMap<TxId, TxRecord>,
}
```

`apply(&mut self, event: Event)` returns `()`. Partner ignores are silent early returns. Do not model `IgnoreReason` (nobody reads it).

Pipeline:

1. Ensure account exists (insert zeroed unlocked). Always create on first named client, including a lone dispute.
2. If `account.locked`, return.
3. Match kind:
   - **Deposit**: ignore if amount <= 0 or `tx` is already in `txs`. Else credit and insert `TxRecord::Deposit { Settled }`.
   - **Withdrawal**: ignore if amount <= 0 or `tx` is already in `txs`. Else insert `TxRecord::Consumed` (failed withdrawal still consumes the id) and debit only when available covers the amount.
   - **Dispute**: load `TxRecord::Deposit` by tx. Ignore if missing, `Consumed`, wrong client, not `Settled`, or available < amount. Else hold and set `Disputed`.
   - **Resolve**: load deposit. Ignore if missing, wrong client, or not `Disputed`. Else release and set `Settled`.
   - **Chargeback**: load deposit. Ignore if missing, wrong client, or not `Disputed`. Else drop held and total, set `ChargedBack`, lock the account.
4. After a successful mutation, `debug_assert` available >= 0, held >= 0, and `total() == available + held`.

`accounts(&self)` yields `&Account` (or owned snapshots). Output is a walk of the accounts map; order is unspecified.

Why one `txs` map: the brief says tx ids are globally unique and failed withdrawals still consume a unique id. A deposit-only map would let a later deposit reuse a withdrawal's tx. Collapsing uniqueness into `txs` avoids storing successful deposit ids twice. Zero/negative amounts and locked-account events do **not** insert (they never become a principal).

Alternative: two maps (`deposits` + `seen_txs`). Rejected; deposit ids were duplicated and the split existed only to mark consumed withdrawals.

### 5. CSVConsumer and StdoutProducer

`CSVConsumer::new(path)` stores the path. `iter()` opens the file with `csv::ReaderBuilder` and returns `Result<impl Iterator<Item = Event>, IoError>`. Open/read setup failure is a process error (`IoError::Io`). Unknown type, missing amount, unparsable numbers, and other bad rows are skipped inside the reader. Do not model `RowError` as a taxonomy a caller matches on.

`StdoutProducer::new()` has no args. `write(iter)` serializes account records to stdout via `csv::Writer` and returns `Result<(), IoError>` (`IoError::Csv` on write failure). Money is capped at four decimal places then printed with trailing zeros omitted (`rescale(4)` then `normalize().to_string()`). Ledger math stays at scale 4. `anyhow` stays in `main` only.

`run(path)`:

```
let mut engine = Engine::new();
let consumer = CSVConsumer::new(path);
for event in consumer.iter()? {
    engine.apply(event);
}
StdoutProducer::new().write(engine.accounts())?;
```

`main` takes `std::env::args().nth(1)` and calls `run`. Missing path may panic or return anyhow; unreadable file returns anyhow and non-zero exit.

### 6. Tests

- Engine unit tests (under `src/engine/` or `src/engine/engine.rs`) take a `Vec<Event>` and assert snapshots. No stdout.
- Invariant helper: after every `apply` in those tests, check available/held/total/lock stickiness and held == sum of Disputed deposits for that client.
- Integration tests in `tests/` use CSV fixtures. Replace the placeholder. Keep `tests/test_data/sample.csv` as the brief sample.
- Do not add `proptest` unless a small randomized sequence helper is cheap; table-driven sequences plus the invariant helper cover 8.5. If added, it is a dev-dependency only.

## Risks / Trade-offs

- **[Risk] Empty accounts in output** → Creating on first named event (including failed dispute) may emit a zeros row. Spec allows this; keep it consistent.
- **[Risk] Memory of consumed withdrawals** → The `txs` map stores a `Consumed` marker per withdrawal id, not the amount. Deposit ids are stored once.
- **[Risk] Locked vs uniqueness** → A deposit on a locked account is ignored and does not consume `tx`. Unlikely partner case; documented here.
- **[Risk] CSV trim/flexible still fails odd quoting** → Skip the row and continue; never abort the stream for a bad row.
- **[Risk] Invariant bugs** → `debug_assert` in apply; tests re-check after every event.

## Migration Plan

Greenfield fill-in of empty modules. No production data. Rollback is unused; the crate has no release.

1. Types and errors.
2. Engine + unit tests.
3. CSV I/O + stdout writer.
4. CLI `run`.
5. Integration fixtures.

## Open Questions

None. Account-creation-on-invalid-dispute, collapsed `txs` uniqueness, csv+serde vs clap, and empty-account output are decided above and reflected in the specs.
