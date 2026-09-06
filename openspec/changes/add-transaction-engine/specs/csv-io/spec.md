## Purpose

Stream payment events from a CSV file into typed records and emit every client's account snapshot as CSV, without loading the whole event list into memory.

## ADDED Requirements

### Requirement: Input CSV event stream

The reader MUST accept a CSV with headers `type`, `client`, `tx`, and `amount`. `type` MUST be one of `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback` (case-sensitive as given). `client` MUST parse as an unsigned 16-bit integer. `tx` MUST parse as an unsigned 32-bit integer. `amount` MUST be a decimal with at most four places after the point, present on deposit and withdrawal, and absent on dispute, resolve, and chargeback. Events MUST be yielded in file order. Whitespace around fields and decimal precision from zero to four places MUST be accepted. The reader MUST stream rows and MUST NOT require the whole file in RAM.

#### Scenario: Brief sample file parses in order

- **WHEN** the sample CSV with five events is read
- **THEN** five events are yielded in file order with clients 1, 2, 1, 1, 2 and amounts 1.0, 2.0, 2.0, 1.5, 3.0 on the first four amount-bearing rows (the last withdrawal has amount 3.0)

#### Scenario: Whitespace around fields

- **WHEN** a row has spaces around type, client, tx, or amount
- **THEN** the row is accepted as a valid event

#### Scenario: Varying decimal precision

- **WHEN** amounts are written as `1`, `1.0`, `1.00`, `1.000`, or `1.0000`
- **THEN** each is accepted as the same value 1.0000

### Requirement: Malformed input rows are skipped

The reader MUST skip a row and continue when the type is unknown, required fields cannot be parsed, a required amount is missing on deposit or withdrawal, or numbers are unparsable. Skipping a row MUST NOT abort the stream.

#### Scenario: Unknown type

- **WHEN** a row has type `transfer`
- **THEN** that row is skipped and later rows are still read

#### Scenario: Missing amount on deposit

- **WHEN** a deposit row has an empty amount
- **THEN** that row is skipped

#### Scenario: Unparsable number

- **WHEN** client, tx, or amount is not a valid number
- **THEN** that row is skipped

### Requirement: Output CSV account snapshots

The writer MUST emit CSV with columns `client`, `available`, `held`, `total`, `locked`. Money fields MUST print at most four decimal places and MUST omit trailing zeros (integers print without a decimal point). Row order MUST NOT matter. Every account known to the engine MUST appear once.

#### Scenario: Brief sample output

- **WHEN** snapshots for client 1 (available 1.5, held 0, total 1.5, locked false) and client 2 (available 2.0, held 0, total 2.0, locked false) are written
- **THEN** the CSV contains those two rows in any order with money printed as `1.5`, `0`, and `2` (trailing zeros omitted)
