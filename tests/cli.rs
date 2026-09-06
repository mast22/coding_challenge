use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rust_challenge"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join(name)
}

fn dec(s: &str) -> Decimal {
    let mut amount = Decimal::from_str(s).expect("valid test amount");
    amount.rescale(4);
    amount
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
fn brief_spaced_sample_matches_brief_output() {
    let snapshots = run_ok("brief_spaced.csv");
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
fn dispute_fixture_holds_funds() {
    let snapshots = run_ok("dispute.csv");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("1.0"), dec("1.0"), false))
    );
}

#[test]
fn resolve_fixture_releases_hold() {
    let snapshots = run_ok("resolve.csv");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("1.0"), dec("0"), dec("1.0"), false))
    );
}

#[test]
fn chargeback_fixture_locks_and_zeroes_held() {
    let snapshots = run_ok("chargeback.csv");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("0"), dec("0"), true))
    );
}

#[test]
fn lock_fixture_freezes_charged_back_client() {
    let snapshots = run_ok("lock.csv");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("0"), dec("0"), true))
    );
    assert_eq!(
        snapshots.get(&2),
        Some(&(dec("2.0"), dec("0"), dec("2.0"), false))
    );
}

#[test]
fn chargeback_remaining_available_is_unchanged_by_reversal() {
    let snapshots = run_ok("chargeback_remaining.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("5.0"), dec("0"), dec("5.0"), true))
    );
}

#[test]
fn concurrent_disputes_cli() {
    let snapshots = run_ok("concurrent_disputes.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("15.0"), dec("15.0"), false))
    );
}

#[test]
fn two_disputes_one_chargeback_cli() {
    let snapshots = run_ok("two_disputes_one_chargeback.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("5.0"), dec("5.0"), true))
    );
}

#[test]
fn lifecycle_cli_ends_locked() {
    let snapshots = run_ok("lifecycle.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("0"), dec("0"), true))
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

#[test]
fn first_dispute_cli_emits_zero_row() {
    let snapshots = run_ok("first_dispute.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("0"), dec("0"), dec("0"), false))
    );
}

#[test]
fn partner_errors_cli() {
    let snapshots = run_ok("partner_errors.csv");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("8.0"), dec("0"), dec("8.0"), false))
    );
    assert_eq!(
        snapshots.get(&2),
        Some(&(dec("0"), dec("0"), dec("0"), false))
    );
}

#[test]
fn global_unique_tx_cli() {
    let snapshots = run_ok("global_unique_tx.csv");
    assert_eq!(
        snapshots.get(&1),
        Some(&(dec("1.0"), dec("0"), dec("1.0"), false))
    );
    assert_eq!(
        snapshots.get(&2),
        Some(&(dec("0"), dec("0"), dec("0"), false))
    );
}
