use std::collections::HashMap;

use crate::types::{Account, Amount, ClientId, Event, TxId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepositState {
    Settled,
    Disputed,
    ChargedBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DepositRecord {
    client: ClientId,
    amount: Amount,
    state: DepositState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TxRecord {
    Deposit(DepositRecord),
    Consumed,
}

pub struct Engine {
    accounts: HashMap<ClientId, Account>,
    txs: HashMap<TxId, TxRecord>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            txs: HashMap::new(),
        }
    }

    pub fn apply(&mut self, event: Event) {
        let client = event.client();
        self.accounts
            .entry(client)
            .or_insert_with(|| Account::new(client));
        if self.accounts[&client].locked {
            return;
        }

        match event {
            Event::Deposit { client, tx, amount } => self.deposit(client, tx, amount),
            Event::Withdrawal { client, tx, amount } => self.withdrawal(client, tx, amount),
            Event::Dispute { client, tx } => self.dispute(client, tx),
            Event::Resolve { client, tx } => self.resolve(client, tx),
            Event::Chargeback { client, tx } => self.chargeback(client, tx),
        }
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    fn deposit(&mut self, client: ClientId, tx: TxId, amount: Amount) {
        if amount <= Amount::ZERO || self.txs.contains_key(&tx) {
            return;
        }

        {
            let account = self.accounts.get_mut(&client).expect("account exists");
            account.available += amount;
            debug_assert_account(account);
        }

        self.txs.insert(
            tx,
            TxRecord::Deposit(DepositRecord {
                client,
                amount,
                state: DepositState::Settled,
            }),
        );
    }

    fn withdrawal(&mut self, client: ClientId, tx: TxId, amount: Amount) {
        if amount <= Amount::ZERO || self.txs.contains_key(&tx) {
            return;
        }

        self.txs.insert(tx, TxRecord::Consumed);

        let available = self.accounts[&client].available;
        if available < amount {
            return;
        }

        let account = self.accounts.get_mut(&client).expect("account exists");
        account.available -= amount;
        debug_assert_account(account);
    }

    fn dispute(&mut self, client: ClientId, tx: TxId) {
        let Some(amount) = self.require_deposit(tx, client, DepositState::Settled) else {
            return;
        };

        {
            let account = self.accounts.get_mut(&client).expect("account exists");
            if account.available < amount {
                return;
            }
            account.available -= amount;
            account.held += amount;
            debug_assert_account(account);
        }

        self.deposit_record_mut(tx).state = DepositState::Disputed;
    }

    fn resolve(&mut self, client: ClientId, tx: TxId) {
        let Some(amount) = self.require_deposit(tx, client, DepositState::Disputed) else {
            return;
        };

        {
            let account = self.accounts.get_mut(&client).expect("account exists");
            account.held -= amount;
            account.available += amount;
            debug_assert_account(account);
        }

        self.deposit_record_mut(tx).state = DepositState::Settled;
    }

    fn chargeback(&mut self, client: ClientId, tx: TxId) {
        let Some(amount) = self.require_deposit(tx, client, DepositState::Disputed) else {
            return;
        };

        {
            let account = self.accounts.get_mut(&client).expect("account exists");
            account.held -= amount;
            account.locked = true;
            debug_assert_account(account);
        }

        self.deposit_record_mut(tx).state = DepositState::ChargedBack;
    }

    fn require_deposit(
        &self,
        tx: TxId,
        client: ClientId,
        expected: DepositState,
    ) -> Option<Amount> {
        match self.txs.get(&tx) {
            Some(TxRecord::Deposit(record))
                if record.client == client && record.state == expected =>
            {
                Some(record.amount)
            }
            _ => None,
        }
    }

    fn deposit_record_mut(&mut self, tx: TxId) -> &mut DepositRecord {
        match self.txs.get_mut(&tx) {
            Some(TxRecord::Deposit(record)) => record,
            _ => panic!("deposit {tx} exists"),
        }
    }
}

fn debug_assert_account(account: &Account) {
    debug_assert!(account.available >= Amount::ZERO);
    debug_assert!(account.held >= Amount::ZERO);
    debug_assert_eq!(account.total(), account.available + account.held);
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::test_utils::dec;

    fn deposit(client: ClientId, tx: TxId, amount: &str) -> Event {
        Event::Deposit {
            client,
            tx,
            amount: dec(amount),
        }
    }

    fn withdrawal(client: ClientId, tx: TxId, amount: &str) -> Event {
        Event::Withdrawal {
            client,
            tx,
            amount: dec(amount),
        }
    }

    fn dispute(client: ClientId, tx: TxId) -> Event {
        Event::Dispute { client, tx }
    }

    fn resolve(client: ClientId, tx: TxId) -> Event {
        Event::Resolve { client, tx }
    }

    fn chargeback(client: ClientId, tx: TxId) -> Event {
        Event::Chargeback { client, tx }
    }

    fn deposit_of(engine: &Engine, tx: TxId) -> &DepositRecord {
        match engine.txs.get(&tx) {
            Some(TxRecord::Deposit(record)) => record,
            other => panic!("expected deposit {tx}, got {other:?}"),
        }
    }

    fn is_deposit(engine: &Engine, tx: TxId) -> bool {
        matches!(engine.txs.get(&tx), Some(TxRecord::Deposit(_)))
    }

    fn deposits(engine: &Engine) -> impl Iterator<Item = (TxId, &DepositRecord)> {
        engine.txs.iter().filter_map(|(tx, record)| match record {
            TxRecord::Deposit(deposit) => Some((*tx, deposit)),
            TxRecord::Consumed => None,
        })
    }

    #[derive(Default)]
    struct InvariantWatch {
        frozen: HashMap<ClientId, Account>,
    }

    fn assert_invariants(engine: &Engine, watch: &mut InvariantWatch) {
        for (tx, record) in deposits(engine) {
            assert!(record.amount > Amount::ZERO);
            assert!(
                engine.accounts.contains_key(&record.client),
                "deposit tx {tx} references missing client {}",
                record.client
            );
        }

        let mut charged_back_clients = HashSet::new();
        for (_, record) in deposits(engine) {
            if record.state == DepositState::ChargedBack {
                charged_back_clients.insert(record.client);
            }
        }

        for account in engine.accounts.values() {
            assert!(account.available >= Amount::ZERO);
            assert!(account.held >= Amount::ZERO);
            assert_eq!(account.total(), account.available + account.held);
            assert_eq!(
                account.available,
                account.total() - account.held,
                "available must equal total - held for client {}",
                account.client
            );
            assert_eq!(
                account.held,
                account.total() - account.available,
                "held must equal total - available for client {}",
                account.client
            );

            let disputed_held = deposits(engine)
                .filter(|(_, record)| {
                    record.client == account.client && record.state == DepositState::Disputed
                })
                .map(|(_, record)| record.amount)
                .fold(Amount::ZERO, |sum, amount| sum + amount);
            assert_eq!(
                account.held, disputed_held,
                "held must equal sum of disputed deposits for client {}",
                account.client
            );

            let has_chargeback = charged_back_clients.contains(&account.client);
            assert_eq!(
                account.locked, has_chargeback,
                "locked iff this client has a charged-back deposit (client {})",
                account.client
            );

            if let Some(prev) = watch.frozen.get(&account.client) {
                assert!(
                    account.locked,
                    "locked must be sticky for client {}",
                    account.client
                );
                assert_eq!(
                    account.available, prev.available,
                    "available must freeze after chargeback for client {}",
                    account.client
                );
                assert_eq!(
                    account.held, prev.held,
                    "held must freeze after chargeback for client {}",
                    account.client
                );
                assert_eq!(
                    account.locked, prev.locked,
                    "locked flag must freeze after chargeback for client {}",
                    account.client
                );
            }
            if account.locked {
                watch
                    .frozen
                    .entry(account.client)
                    .or_insert_with(|| account.clone());
            }
        }
    }

    fn apply_checked(engine: &mut Engine, event: Event, watch: &mut InvariantWatch) {
        engine.apply(event);
        assert_invariants(engine, watch);
    }

    fn apply_all(events: impl IntoIterator<Item = Event>) -> Engine {
        let mut engine = Engine::new();
        let mut watch = InvariantWatch::default();
        for event in events {
            apply_checked(&mut engine, event, &mut watch);
        }
        engine
    }

    fn assert_snapshot(
        engine: &Engine,
        client: ClientId,
        available: &str,
        held: &str,
        total: &str,
        locked: bool,
    ) {
        let account = engine
            .accounts
            .get(&client)
            .unwrap_or_else(|| panic!("missing account {client}"));
        assert_eq!(account.available, dec(available));
        assert_eq!(account.held, dec(held));
        assert_eq!(account.total(), dec(total));
        assert_eq!(account.locked, locked);
    }

    #[test]
    fn deposit_credits_available() {
        let engine = apply_all([deposit(1, 1, "1.0")]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
    }

    #[test]
    fn two_deposits_sum() {
        let engine = apply_all([deposit(1, 1, "1.0"), deposit(1, 2, "2.0")]);
        assert_snapshot(&engine, 1, "3.0000", "0", "3.0000", false);
    }

    #[test]
    fn withdraw_success() {
        let engine = apply_all([deposit(1, 1, "2.0"), withdrawal(1, 2, "1.5")]);
        assert_snapshot(&engine, 1, "0.5000", "0", "0.5000", false);
    }

    #[test]
    fn withdraw_insufficient() {
        let engine = apply_all([deposit(1, 1, "2.0"), withdrawal(1, 2, "3.0")]);
        assert_snapshot(&engine, 1, "2.0000", "0", "2.0000", false);
    }

    #[test]
    fn withdraw_exact() {
        let engine = apply_all([deposit(1, 1, "1.0"), withdrawal(1, 2, "1.0")]);
        assert_snapshot(&engine, 1, "0", "0", "0", false);
    }

    #[test]
    fn precision_subtract() {
        let engine = apply_all([deposit(1, 1, "1.2345"), withdrawal(1, 2, "0.0001")]);
        assert_snapshot(&engine, 1, "1.2344", "0", "1.2344", false);
    }

    #[test]
    fn first_event_withdrawal_creates_zeros_account() {
        let engine = apply_all([withdrawal(1, 1, "1.0")]);
        assert_snapshot(&engine, 1, "0", "0", "0", false);
    }

    #[test]
    fn interleaved_brief_sample() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            deposit(2, 2, "2.0"),
            deposit(1, 3, "2.0"),
            withdrawal(1, 4, "1.5"),
            withdrawal(2, 5, "3.0"),
        ]);
        assert_snapshot(&engine, 1, "1.5000", "0", "1.5000", false);
        assert_snapshot(&engine, 2, "2.0000", "0", "2.0000", false);
    }

    #[test]
    fn zero_or_negative_amounts_ignored() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            deposit(1, 2, "0"),
            deposit(1, 3, "-1"),
            withdrawal(1, 4, "0"),
            withdrawal(1, 5, "-1"),
        ]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert!(!engine.txs.contains_key(&2));
        assert!(!engine.txs.contains_key(&3));
        assert!(!engine.txs.contains_key(&4));
        assert!(!engine.txs.contains_key(&5));
    }

    #[test]
    fn duplicate_deposit_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1"), deposit(1, 1, "5")]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
    }

    #[test]
    fn four_decimal_exact_sum() {
        let engine = apply_all([deposit(1, 1, "1.0001"), deposit(1, 2, "2.0002")]);
        assert_snapshot(&engine, 1, "3.0003", "0", "3.0003", false);
    }

    #[test]
    fn failed_withdrawal_consumes_tx_id() {
        let engine = apply_all([
            deposit(1, 1, "2.0"),
            withdrawal(1, 2, "3.0"),
            deposit(1, 2, "1.0"),
        ]);
        assert_snapshot(&engine, 1, "2.0000", "0", "2.0000", false);
        assert!(engine.txs.contains_key(&2));
        assert!(!is_deposit(&engine, 2));
    }

    #[test]
    fn dispute_moves_to_held() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1)]);
        assert_snapshot(&engine, 1, "0", "1.0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
    }

    #[test]
    fn resolve_restores() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), resolve(1, 1)]);
        assert_snapshot(&engine, 1, "1.0", "0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn chargeback_freezes() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), chargeback(1, 1)]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn dispute_one_of_two_deposits() {
        let engine = apply_all([deposit(1, 1, "1"), deposit(1, 2, "2"), dispute(1, 1)]);
        assert_snapshot(&engine, 1, "2", "1", "3", false);
    }

    #[test]
    fn double_dispute_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), dispute(1, 1)]);
        assert_snapshot(&engine, 1, "0", "1.0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
    }

    #[test]
    fn re_dispute_after_resolve() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            resolve(1, 1),
            dispute(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "1.0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
    }

    #[test]
    fn cannot_re_dispute_after_chargeback() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            chargeback(1, 1),
            dispute(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn resolve_then_chargeback_ignored() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            resolve(1, 1),
            chargeback(1, 1),
        ]);
        assert_snapshot(&engine, 1, "1.0", "0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn chargeback_then_resolve_ignored() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            chargeback(1, 1),
            resolve(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn withdraw_while_held_fails() {
        let engine = apply_all([deposit(1, 1, "10"), dispute(1, 1), withdrawal(1, 2, "1")]);
        assert_snapshot(&engine, 1, "0", "10", "10", false);
        assert!(engine.txs.contains_key(&2));
    }

    #[test]
    fn withdraw_after_resolve() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            dispute(1, 1),
            resolve(1, 1),
            withdrawal(1, 2, "4"),
        ]);
        assert_snapshot(&engine, 1, "6", "0", "6", false);
    }

    #[test]
    fn locked_account_ignores_further_events() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            chargeback(1, 1),
            deposit(1, 2, "5.0"),
            withdrawal(1, 3, "1.0"),
            dispute(1, 1),
            resolve(1, 1),
            chargeback(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert!(!engine.txs.contains_key(&2));
        assert!(!engine.txs.contains_key(&3));
        assert!(!is_deposit(&engine, 2));
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn dispute_unknown_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 999)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
        assert!(!is_deposit(&engine, 999));
    }

    #[test]
    fn resolve_unknown_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), resolve(1, 999)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn chargeback_unknown_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), chargeback(1, 999)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert!(!engine.accounts[&1].locked);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn resolve_on_undisputed_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), resolve(1, 1)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn chargeback_on_undisputed_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), chargeback(1, 1)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert!(!engine.accounts[&1].locked);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn dispute_withdrawal_tx_ignored() {
        let engine = apply_all([deposit(1, 1, "10"), withdrawal(1, 2, "2"), dispute(1, 2)]);
        assert_snapshot(&engine, 1, "8", "0", "8", false);
        assert!(!is_deposit(&engine, 2));
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn wrong_client_dispute_ignored() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(2, 1)]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_snapshot(&engine, 2, "0", "0", "0", false);
        assert_eq!(deposit_of(&engine, 1).client, 1);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn dispute_after_partial_spend_ignored() {
        let engine = apply_all([deposit(1, 1, "10"), withdrawal(1, 2, "6"), dispute(1, 1)]);
        assert_snapshot(&engine, 1, "4", "0", "4", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn stream_order_tx_5_before_tx_1() {
        // Sorting by tx would apply the withdrawal first (fail) then the deposit.
        let engine = apply_all([deposit(1, 5, "5.0"), withdrawal(1, 1, "3.0")]);
        assert_snapshot(&engine, 1, "2.0000", "0", "2.0000", false);
        assert!(is_deposit(&engine, 5));
        assert!(!is_deposit(&engine, 1));
        assert!(engine.txs.contains_key(&5));
        assert!(engine.txs.contains_key(&1));
    }

    #[test]
    fn first_event_dispute_creates_zeros_account() {
        let engine = apply_all([dispute(1, 1)]);
        assert_snapshot(&engine, 1, "0", "0", "0", false);
        assert!(deposits(&engine).next().is_none());
    }

    #[test]
    fn first_event_resolve_creates_zeros_account() {
        let engine = apply_all([resolve(1, 1)]);
        assert_snapshot(&engine, 1, "0", "0", "0", false);
        assert!(!engine.accounts[&1].locked);
    }

    #[test]
    fn first_event_chargeback_creates_zeros_account_unlocked() {
        let engine = apply_all([chargeback(1, 1)]);
        assert_snapshot(&engine, 1, "0", "0", "0", false);
        assert!(!engine.accounts[&1].locked);
    }

    #[test]
    fn dispute_and_resolve_do_not_lock() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1)]);
        assert!(!engine.accounts[&1].locked);
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), resolve(1, 1)]);
        assert!(!engine.accounts[&1].locked);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn wrong_client_resolve_ignored_while_dispute_open() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), resolve(2, 1)]);
        assert_snapshot(&engine, 1, "0", "1.0", "1.0", false);
        assert_snapshot(&engine, 2, "0", "0", "0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
        assert_eq!(deposit_of(&engine, 1).client, 1);
    }

    #[test]
    fn wrong_client_chargeback_ignored_does_not_lock_owner() {
        let engine = apply_all([deposit(1, 1, "1.0"), dispute(1, 1), chargeback(2, 1)]);
        assert_snapshot(&engine, 1, "0", "1.0", "1.0", false);
        assert_snapshot(&engine, 2, "0", "0", "0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
        assert!(!engine.accounts[&1].locked);
        assert!(!engine.accounts[&2].locked);
    }

    #[test]
    fn global_tx_uniqueness_blocks_other_client_deposit() {
        let engine = apply_all([deposit(1, 1, "1.0"), deposit(2, 1, "9.0")]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_snapshot(&engine, 2, "0", "0", "0", false);
        assert_eq!(deposit_of(&engine, 1).client, 1);
        assert!(!deposits(&engine).any(|(_, record)| record.client == 2));
    }

    #[test]
    fn duplicate_withdrawal_tx_ignored() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            withdrawal(1, 2, "1"),
            withdrawal(1, 2, "1"),
        ]);
        assert_snapshot(&engine, 1, "9", "0", "9", false);
    }

    #[test]
    fn failed_withdrawal_tx_blocks_later_deposit() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            withdrawal(1, 2, "5.0"),
            deposit(2, 2, "9.0"),
        ]);
        assert_snapshot(&engine, 1, "1.0000", "0", "1.0000", false);
        assert_snapshot(&engine, 2, "0", "0", "0", false);
        assert!(engine.txs.contains_key(&2));
        assert!(!is_deposit(&engine, 2));
    }

    #[test]
    fn deposit_tx_blocks_later_withdrawal_same_id() {
        let engine = apply_all([deposit(1, 1, "5.0"), withdrawal(1, 1, "1.0")]);
        assert_snapshot(&engine, 1, "5.0000", "0", "5.0000", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn chargeback_leaves_available_from_other_deposits_unchanged() {
        // PDF: available is unchanged by the chargeback step; held and total drop.
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            chargeback(1, 1),
        ]);
        assert_snapshot(&engine, 1, "5", "0", "5", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
        assert_eq!(deposit_of(&engine, 2).state, DepositState::Settled);
    }

    #[test]
    fn two_concurrent_disputes_sum_held() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            dispute(1, 2),
        ]);
        assert_snapshot(&engine, 1, "0", "15", "15", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Disputed);
        assert_eq!(deposit_of(&engine, 2).state, DepositState::Disputed);
    }

    #[test]
    fn two_disputes_one_chargeback_locks_with_remaining_held() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            dispute(1, 2),
            chargeback(1, 1),
            resolve(1, 2),
            chargeback(1, 2),
        ]);
        // First chargeback freezes the account immediately. The still-disputed
        // 5 remains held; later resolve/chargeback of tx 2 are ignored.
        assert_snapshot(&engine, 1, "0", "5", "5", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
        assert_eq!(deposit_of(&engine, 2).state, DepositState::Disputed);
    }

    #[test]
    fn resolve_one_of_two_disputes_releases_only_that_amount() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            dispute(1, 2),
            resolve(1, 1),
        ]);
        assert_snapshot(&engine, 1, "10", "5", "15", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
        assert_eq!(deposit_of(&engine, 2).state, DepositState::Disputed);
    }

    #[test]
    fn lifecycle_resolve_then_redispute_then_chargeback() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            resolve(1, 1),
            dispute(1, 1),
            chargeback(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn withdraw_remaining_available_while_other_deposit_held() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            withdrawal(1, 3, "5"),
        ]);
        assert_snapshot(&engine, 1, "0", "10", "10", false);
        assert!(engine.txs.contains_key(&3));
        assert!(!is_deposit(&engine, 3));
    }

    #[test]
    fn withdraw_more_than_remaining_while_other_deposit_held_fails() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            deposit(1, 2, "5"),
            dispute(1, 1),
            withdrawal(1, 3, "6"),
        ]);
        assert_snapshot(&engine, 1, "5", "10", "15", false);
        assert!(engine.txs.contains_key(&3));
    }

    #[test]
    fn double_resolve_ignored() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            resolve(1, 1),
            resolve(1, 1),
        ]);
        assert_snapshot(&engine, 1, "1.0", "0", "1.0", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
        assert!(!engine.accounts[&1].locked);
    }

    #[test]
    fn double_chargeback_second_ignored() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            chargeback(1, 1),
            chargeback(1, 1),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::ChargedBack);
    }

    #[test]
    fn many_clients_independent_ledgers() {
        let engine = apply_all([
            deposit(1, 1, "1"),
            deposit(2, 2, "2"),
            deposit(3, 3, "3"),
            withdrawal(2, 4, "1"),
            dispute(3, 3),
            chargeback(3, 3),
            deposit(1, 5, "4"),
        ]);
        assert_snapshot(&engine, 1, "5", "0", "5", false);
        assert_snapshot(&engine, 2, "1", "0", "1", false);
        assert_snapshot(&engine, 3, "0", "0", "0", true);
    }

    #[test]
    fn locked_event_does_not_consume_tx_used_by_other_client() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            dispute(1, 1),
            chargeback(1, 1),
            deposit(1, 99, "5.0"),
            deposit(2, 99, "7.0"),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_snapshot(&engine, 2, "7.0", "0", "7.0", false);
        assert_eq!(deposit_of(&engine, 99).client, 2);
        assert!(engine.txs.contains_key(&99));
    }

    #[test]
    fn chargeback_of_one_client_does_not_lock_another() {
        let engine = apply_all([
            deposit(1, 1, "1.0"),
            deposit(2, 2, "2.0"),
            dispute(1, 1),
            chargeback(1, 1),
            withdrawal(2, 3, "0.5"),
        ]);
        assert_snapshot(&engine, 1, "0", "0", "0", true);
        assert_snapshot(&engine, 2, "1.5", "0", "1.5", false);
    }

    #[test]
    fn client_zero_and_max_ids() {
        let engine = apply_all([
            deposit(0, 0, "1.0000"),
            deposit(u16::MAX, u32::MAX, "2.0000"),
            dispute(0, 0),
            resolve(0, 0),
            dispute(u16::MAX, u32::MAX),
            chargeback(u16::MAX, u32::MAX),
        ]);
        assert_snapshot(&engine, 0, "1.0000", "0", "1.0000", false);
        assert_snapshot(&engine, u16::MAX, "0", "0", "0", true);
    }

    #[test]
    fn dispute_amount_comes_from_stored_deposit() {
        let engine = apply_all([deposit(1, 1, "3.1415"), dispute(1, 1)]);
        assert_snapshot(&engine, 1, "0", "3.1415", "3.1415", false);
        assert_eq!(deposit_of(&engine, 1).amount, dec("3.1415"));
    }

    #[test]
    fn resolve_after_second_dispute_releases_again() {
        let engine = apply_all([
            deposit(1, 1, "2.5"),
            dispute(1, 1),
            resolve(1, 1),
            dispute(1, 1),
            resolve(1, 1),
        ]);
        assert_snapshot(&engine, 1, "2.5", "0", "2.5", false);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
    }

    #[test]
    fn withdrawals_are_never_stored_as_disputable_principals() {
        let engine = apply_all([
            deposit(1, 1, "10"),
            withdrawal(1, 2, "3"),
            withdrawal(1, 3, "99"),
        ]);
        assert_eq!(deposits(&engine).count(), 1);
        assert!(is_deposit(&engine, 1));
        assert!(!is_deposit(&engine, 2));
        assert!(!is_deposit(&engine, 3));
        assert!(engine.txs.contains_key(&2));
        assert!(engine.txs.contains_key(&3));
    }

    #[test]
    fn long_mixed_stream_preserves_invariants() {
        let engine = apply_all([
            deposit(1, 1, "10.0001"),
            deposit(2, 2, "0.0001"),
            deposit(1, 3, "2.0002"),
            withdrawal(1, 4, "1.0000"),
            dispute(1, 3),
            resolve(1, 3),
            dispute(1, 3),
            withdrawal(2, 5, "0.0002"),
            deposit(3, 6, "8"),
            dispute(3, 6),
            chargeback(3, 6),
            deposit(3, 7, "100"),
            deposit(1, 8, "0.5"),
            withdrawal(1, 9, "0.25"),
            chargeback(1, 3),
            resolve(1, 1),
            dispute(2, 2),
        ]);
        assert_snapshot(&engine, 1, "9.2501", "0", "9.2501", true);
        assert_snapshot(&engine, 2, "0", "0.0001", "0.0001", false);
        assert_snapshot(&engine, 3, "0", "0", "0", true);
        assert_eq!(deposit_of(&engine, 3).state, DepositState::ChargedBack);
        assert_eq!(deposit_of(&engine, 1).state, DepositState::Settled);
        assert_eq!(deposit_of(&engine, 2).state, DepositState::Disputed);
    }
}
