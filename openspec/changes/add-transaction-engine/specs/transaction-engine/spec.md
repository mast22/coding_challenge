## Purpose

Process a chronological stream of payment events for many clients and keep a correct per-client ledger of available, held, total, and locked funds.

## ADDED Requirements

### Requirement: Sequential processing and account creation

The engine MUST process events strictly in stream order and MUST NOT reorder by transaction id. Client ids need not appear in numeric order. The first event that names a client MUST create that client's account with available 0, held 0, total 0, and locked false. Ledgers for distinct clients MUST be independent.

#### Scenario: Interleaved clients from the brief sample

- **WHEN** the engine applies deposit client 1 tx 1 amount 1.0, deposit client 2 tx 2 amount 2.0, deposit client 1 tx 3 amount 2.0, withdrawal client 1 tx 4 amount 1.5, withdrawal client 2 tx 5 amount 3.0
- **THEN** client 1 has available 1.5000, held 0, total 1.5000, locked false
- **AND** client 2 has available 2.0000, held 0, total 2.0000, locked false

#### Scenario: Chronological order is not numeric order

- **WHEN** events arrive with tx 5 before tx 1
- **THEN** they are applied in stream order, not sorted by tx

#### Scenario: First event for a client is a withdrawal

- **WHEN** the first event for a client is a withdrawal
- **THEN** the account is created at zeros and the withdrawal fails, leaving available 0, held 0, total 0, locked false

#### Scenario: First event for a client is a dispute

- **WHEN** the first event for a client is a dispute of an unknown tx
- **THEN** an account for that client exists at zeros, unlocked, with no mutation

#### Scenario: Many clients created independently

- **WHEN** events name several distinct clients
- **THEN** each ledger is independent and there is no cross-talk

### Requirement: Deposit credits

A deposit MUST credit available and total by the given amount when the amount is greater than zero and the tx has not already been used for a deposit or withdrawal. A successful deposit MUST be stored as a disputable principal in Settled state. Zero or negative amounts MUST be ignored. A second deposit or withdrawal with an already-seen tx MUST be ignored.

#### Scenario: Deposit credits

- **WHEN** deposit client 1 tx 1 amount 1.0 is applied
- **THEN** client 1 has available 1.0000, held 0, total 1.0000, locked false

#### Scenario: Two deposits same client

- **WHEN** deposit 1.0 then deposit 2.0 are applied to the same client
- **THEN** available is 3.0000, held is 0, total is 3.0000

#### Scenario: Duplicate deposit tx

- **WHEN** deposit tx 1 amount 1 is followed by deposit tx 1 amount 5
- **THEN** only the first credit is applied

#### Scenario: Zero or negative deposit

- **WHEN** a deposit of 0 or a deposit of -1 is applied
- **THEN** the account is unchanged

### Requirement: Withdrawal debits

A withdrawal MUST debit available and total by the amount when available is greater than or equal to the amount and the amount is greater than zero. If available is insufficient, the withdrawal MUST be a no-op and MUST NOT be stored as a disputable credit. Failed withdrawals that have a positive amount MUST still consume the tx id so a later deposit or withdrawal with that tx is ignored. Zero or negative amounts MUST be ignored.

#### Scenario: Withdraw success

- **WHEN** deposit 2.0 then withdrawal 1.5 are applied
- **THEN** available is 0.5000, held is 0, total is 0.5000

#### Scenario: Withdraw insufficient

- **WHEN** deposit 2.0 then withdrawal 3.0 are applied
- **THEN** available remains 2.0000

#### Scenario: Withdraw exact

- **WHEN** deposit 1.0 then withdrawal 1.0 are applied
- **THEN** available, held, and total are 0 and the account is unlocked

#### Scenario: Withdraw on unknown client

- **WHEN** a withdrawal is the first event for a client
- **THEN** the account exists at zeros, unlocked

#### Scenario: Zero or negative withdrawal

- **WHEN** a withdrawal of 0 or a negative amount is applied
- **THEN** the account is unchanged

### Requirement: Dispute holds a settled deposit

A dispute MUST look up the referenced tx. On success it MUST decrease available and increase held by the original deposit amount, leave total unchanged, and mark that deposit Disputed. The dispute event itself has no amount; the amount MUST come from the stored deposit. A given tx MUST be under at most one open dispute at a time.

#### Scenario: Dispute moves to held

- **WHEN** deposit 1.0 is followed by a dispute of that tx
- **THEN** available is 0, held is 1.0, total is 1.0, locked is false

#### Scenario: Dispute one of two deposits

- **WHEN** deposit A amount 1, deposit B amount 2, then dispute A
- **THEN** available is 2, held is 1, total is 3

#### Scenario: Double dispute

- **WHEN** deposit, dispute, then a second dispute of the same tx
- **THEN** held is still one deposit amount

### Requirement: Resolve releases a dispute

A resolve MUST look up the referenced tx. If the deposit is currently Disputed and the client matches, held MUST decrease by the amount, available MUST increase by the amount, total MUST be unchanged, and the deposit MUST return to Settled. Re-dispute after resolve MUST be allowed.

#### Scenario: Resolve restores

- **WHEN** deposit 1.0, dispute, then resolve of that tx
- **THEN** available is 1.0, held is 0, locked is false

#### Scenario: Re-dispute after resolve

- **WHEN** deposit, dispute, resolve, then dispute again on the same deposit
- **THEN** funds are held again

#### Scenario: Withdraw after resolve

- **WHEN** deposit 10, dispute, resolve, then withdrawal 4
- **THEN** available is 6

### Requirement: Chargeback reverses a dispute and locks the account

A chargeback MUST look up the referenced tx. If the deposit is currently Disputed and the client matches, held MUST decrease by the amount, total MUST decrease by the amount, available MUST be unchanged by this step, the deposit MUST become ChargedBack, and locked MUST be set to true immediately. ChargedBack is terminal for that tx. Re-dispute after chargeback MUST be ignored.

#### Scenario: Chargeback freezes

- **WHEN** deposit 1.0, dispute, then chargeback of that tx
- **THEN** available is 0, held is 0, total is 0, locked is true

#### Scenario: Cannot re-dispute after chargeback

- **WHEN** deposit, dispute, chargeback, then dispute of that tx
- **THEN** the dispute is ignored and the snapshot stays frozen

#### Scenario: Resolve after chargeback of the same tx

- **WHEN** deposit, dispute, chargeback, then resolve of that tx
- **THEN** the resolve is ignored and the account stays locked

#### Scenario: Resolve then chargeback

- **WHEN** deposit, dispute, resolve, then chargeback of that tx
- **THEN** the chargeback is ignored because the tx is no longer disputed

### Requirement: Locked accounts reject all subsequent mutations

After locked is true, the engine MUST ignore further deposits, withdrawals, disputes, resolves, and chargebacks on that client. Locked MUST be sticky: once true, it stays true.

#### Scenario: Deposit after lock

- **WHEN** deposit, dispute, chargeback, then a second deposit on the same client
- **THEN** the second deposit is ignored and totals are unchanged

#### Scenario: Withdraw after lock

- **WHEN** a locked account receives a withdrawal
- **THEN** the withdrawal is ignored

#### Scenario: Any mutation after a chargeback lock

- **WHEN** any later event names the locked client
- **THEN** the snapshot stays frozen

### Requirement: Partner errors are silent ignores

The engine MUST ignore, without changing account state, each of the following partner-side mistakes: unknown tx on dispute, resolve, or chargeback; resolve or chargeback on a tx that is not currently disputed; dispute of a withdrawal tx; dispute of an already-disputed or already-charged-back tx; dispute, resolve, or chargeback whose client id does not match the original deposit's client; dispute when available is less than the original deposit amount.

#### Scenario: Dispute of an unknown tx

- **WHEN** dispute 999 is applied
- **THEN** no account balances change

#### Scenario: Resolve of an unknown tx

- **WHEN** resolve of a missing tx is applied
- **THEN** no account balances change

#### Scenario: Chargeback of an unknown tx

- **WHEN** chargeback of a missing tx is applied
- **THEN** no account balances change

#### Scenario: Resolve on an undisputed tx

- **WHEN** a deposit is followed by resolve without a dispute
- **THEN** the account is unchanged

#### Scenario: Chargeback on an undisputed tx

- **WHEN** a deposit is followed by chargeback without a dispute
- **THEN** the account is unchanged and still unlocked

#### Scenario: Dispute a withdrawal tx

- **WHEN** deposit, withdrawal tx 2, then dispute tx 2
- **THEN** the dispute is ignored

#### Scenario: Wrong client on dispute

- **WHEN** deposit client 1 tx 1 is followed by dispute client 2 tx 1
- **THEN** both accounts are unchanged by the dispute (client 1 remains credited; client 2 is not credited from that deposit)

#### Scenario: Dispute after partial spend

- **WHEN** deposit 10, withdrawal 6, then dispute of the 10
- **THEN** the dispute is ignored because available 4 is less than 10

#### Scenario: Withdraw while held

- **WHEN** deposit 10, dispute, then withdrawal 1
- **THEN** the withdrawal fails because available is 0

### Requirement: Ledger invariants

After every successful mutation, for every account: available MUST be at least 0; held MUST be at least 0; total MUST equal available plus held. The sum of held across an account MUST equal the sum of amounts of that client's deposits in Disputed state. Money values MUST be exact to four decimal places and MUST NOT use binary floating-point for mutation.

#### Scenario: Four-decimal exact sum

- **WHEN** deposit 1.0001 then deposit 2.0002
- **THEN** total is 3.0003

#### Scenario: Precision subtract

- **WHEN** deposit 1.2345 then withdrawal 0.0001
- **THEN** available and total are 1.2344

#### Scenario: Invariants after every apply

- **WHEN** any successful mutation is applied
- **THEN** available >= 0, held >= 0, available + held = total, and locked once true stays true
