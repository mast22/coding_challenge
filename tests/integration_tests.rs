use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use rust_challenge::{Account, CSVConsumer, Engine};
use rust_decimal::Decimal;

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

fn run_fixture(name: &str) -> HashMap<u16, Account> {
    let mut engine = Engine::new();
    let consumer = CSVConsumer::new(fixture(name));
    for event in consumer.iter().expect("fixture opens") {
        engine.apply(event);
    }
    let snapshots: HashMap<u16, Account> = engine
        .accounts()
        .cloned()
        .map(|account| (account.client(), account))
        .collect();
    for account in snapshots.values() {
        assert!(account.available() >= Decimal::ZERO);
        assert!(account.held() >= Decimal::ZERO);
        assert_eq!(account.total(), account.available() + account.held());
        assert_eq!(account.available(), account.total() - account.held());
        assert_eq!(account.held(), account.total() - account.available());
    }
    snapshots
}

fn assert_account(
    snapshots: &HashMap<u16, Account>,
    client: u16,
    available: &str,
    held: &str,
    total: &str,
    locked: bool,
) {
    let account = snapshots
        .get(&client)
        .unwrap_or_else(|| panic!("missing account {client}"));
    assert_eq!(account.available(), dec(available));
    assert_eq!(account.held(), dec(held));
    assert_eq!(account.total(), dec(total));
    assert_eq!(account.locked(), locked);
}

#[test]
fn brief_sample_csv_matches_brief_snapshots() {
    let snapshots = run_fixture("sample.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "1.5000", "0", "1.5000", false);
    assert_account(&snapshots, 2, "2.0000", "0", "2.0000", false);
}

#[test]
fn dispute_happy_path_moves_deposit_to_held() {
    let snapshots = run_fixture("dispute.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "1.0", "1.0", false);
}

#[test]
fn resolve_happy_path_restores_available() {
    let snapshots = run_fixture("resolve.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "1.0", "0", "1.0", false);
}

#[test]
fn chargeback_happy_path_freezes_account() {
    let snapshots = run_fixture("chargeback.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "0", "0", true);
}

#[test]
fn lock_ignores_later_events_on_charged_back_client() {
    let snapshots = run_fixture("lock.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "0", "0", "0", true);
    assert_account(&snapshots, 2, "2.0", "0", "2.0", false);
}

#[test]
fn malformed_rows_are_skipped_and_valid_clients_are_kept() {
    let snapshots = run_fixture("mixed.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "1.5", "0", "1.5", false);
    assert_account(&snapshots, 2, "2.0", "0", "2.0", false);
}

#[test]
fn brief_sample_with_spaces_matches_compact_sample() {
    let snapshots = run_fixture("brief_spaced.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "1.5000", "0", "1.5000", false);
    assert_account(&snapshots, 2, "2.0000", "0", "2.0000", false);
}

#[test]
fn chargeback_leaves_undisputed_deposit_available() {
    let snapshots = run_fixture("chargeback_remaining.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "5.0", "0", "5.0", true);
}

#[test]
fn concurrent_disputes_hold_both_amounts() {
    let snapshots = run_fixture("concurrent_disputes.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "15.0", "15.0", false);
}

#[test]
fn two_disputes_one_chargeback_freezes_remaining_held() {
    let snapshots = run_fixture("two_disputes_one_chargeback.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "5.0", "5.0", true);
}

#[test]
fn lifecycle_redispute_then_chargeback_locks() {
    let snapshots = run_fixture("lifecycle.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "0", "0", true);
}

#[test]
fn first_event_dispute_emits_zero_unlocked_account() {
    let snapshots = run_fixture("first_dispute.csv");
    assert_eq!(snapshots.len(), 1);
    assert_account(&snapshots, 1, "0", "0", "0", false);
}

#[test]
fn empty_file_emits_no_accounts() {
    let snapshots = run_fixture("empty.csv");
    assert!(snapshots.is_empty());
}

#[test]
fn global_tx_uniqueness_does_not_credit_second_client() {
    let snapshots = run_fixture("global_unique_tx.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "1.0", "0", "1.0", false);
    assert_account(&snapshots, 2, "0", "0", "0", false);
}

#[test]
fn partner_errors_do_not_mutate_valid_balances() {
    let snapshots = run_fixture("partner_errors.csv");
    assert_eq!(snapshots.len(), 2);
    assert_account(&snapshots, 1, "8.0", "0", "8.0", false);
    assert_account(&snapshots, 2, "0", "0", "0", false);
}
