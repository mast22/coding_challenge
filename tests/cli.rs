//! cli.rs in an integration tests suite for CLI interface
//! It checks the system's output and how correctly it was produced

use std::collections::HashMap;
use std::process::Command;

use rust_decimal::Decimal;
use serde::Deserialize;

use rust_challenge::test_utils::{dec, fixture};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rust_challenge"))
}

#[derive(Debug, Deserialize)]
struct SnapshotRow {
    client: u16,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

fn parse_snapshots(csv: &str) -> HashMap<u16, (Decimal, Decimal, Decimal, bool)> {
    let mut reader = csv::Reader::from_reader(csv.as_bytes());
    reader
        .deserialize::<SnapshotRow>()
        .map(|row| {
            let row = row.expect("output row parses");
            (
                row.client,
                (
                    dec(&row.available),
                    dec(&row.held),
                    dec(&row.total),
                    row.locked,
                ),
            )
        })
        .collect()
}

fn run_ok(name: &str) -> HashMap<u16, (Decimal, Decimal, Decimal, bool)> {
    let output = bin().arg(fixture(name)).output().expect("binary runs");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(
        stdout.starts_with("client,available,held,total,locked"),
        "stdout must start with the accounts header, got: {stdout:?}"
    );
    let snapshots = parse_snapshots(&stdout);
    for (client, (available, held, total, _)) in &snapshots {
        assert!(*available >= Decimal::ZERO, "client {client} available");
        assert!(*held >= Decimal::ZERO, "client {client} held");
        assert_eq!(*total, *available + *held, "client {client} total");
    }
    snapshots
}

#[test]
fn sample_csv_prints_brief_account_rows() {
    let output = bin()
        .arg(fixture("sample.csv"))
        .output()
        .expect("binary runs");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("1.5"));
    assert!(stdout.contains("2"));
    assert!(!stdout.contains("1.5000"));
    assert!(!stdout.contains("2.0000"));

    let snapshots = parse_snapshots(&stdout);
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("1.5"), dec("0"), dec("1.5"), false))
    );
    assert_eq!(
        snapshots.get(&2),
        Some(&(dec("2.0"), dec("0"), dec("2.0"), false))
    );
}

#[test]
fn missing_path_is_process_error() {
    let output = bin().output().expect("binary runs");
    assert!(!output.status.success());
}

#[test]
fn unreadable_path_is_process_error() {
    let output = bin()
        .arg("/this/path/does/not/exist.csv")
        .output()
        .expect("binary runs");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty() || !stdout.contains("client,available,held,total,locked"),
        "unreadable path must not write a successful accounts CSV, got: {stdout}"
    );
}

#[test]
fn mixed_valid_and_malformed_rows_write_valid_clients() {
    let snapshots = run_ok("mixed.csv");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("1.5"), dec("0"), dec("1.5"), false))
    );
    assert_eq!(
        snapshots.get(&2),
        Some(&(dec("2.0"), dec("0"), dec("2.0"), false))
    );
}

#[test]
fn empty_csv_writes_header_only() {
    let output = bin()
        .arg(fixture("empty.csv"))
        .output()
        .expect("binary runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert_eq!(stdout, "client,available,held,total,locked\n");
}
