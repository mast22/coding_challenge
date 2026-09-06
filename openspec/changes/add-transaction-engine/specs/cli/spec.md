## Purpose

Provide a single-argument command-line program that reads a transactions CSV and writes account snapshots as CSV to standard output.

## ADDED Requirements

### Requirement: Invoke with one CSV path

The program MUST be invoked as `cargo run -- transactions.csv`. It MUST take exactly one positional argument: the path to the input CSV. Argument parsing MUST use the standard library only and MUST NOT use clap. The program MUST write the resulting accounts CSV to stdout and only stdout for the result.

#### Scenario: Happy path redirect

- **WHEN** the user runs `cargo run -- transactions.csv > accounts.csv` with a valid input file
- **THEN** stdout is a CSV of account snapshots and the process exits successfully

#### Scenario: Missing path

- **WHEN** the program is started with no path argument
- **THEN** it fails as a process error with a non-zero exit

### Requirement: Process errors versus skipped rows

A missing or unreadable input file MUST be a process error (non-zero exit). A bad row, unknown type, or missing required amount MUST skip that row and keep streaming. Partner-semantic mistakes (unknown tx, insufficient funds, locked account, duplicate tx, and the other engine ignores) MUST be silent and MUST NOT require stderr.

#### Scenario: Unreadable file

- **WHEN** the given path cannot be opened
- **THEN** the process exits non-zero and does not write a successful accounts CSV

#### Scenario: Mixed good and bad rows

- **WHEN** the file contains valid events and also malformed rows
- **THEN** valid events are applied, malformed rows are skipped, and snapshots for processed clients are written to stdout
