use std::io::{self, Write};

use serde::Serialize;

use crate::error::IoError;
use crate::types::{Account, Amount, ClientId};

#[derive(Serialize)]
struct OutputRow {
    client: ClientId,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

impl From<&Account> for OutputRow {
    fn from(account: &Account) -> Self {
        Self {
            client: account.client,
            available: format_amount(account.available),
            held: format_amount(account.held),
            total: format_amount(account.total()),
            locked: account.locked,
        }
    }
}

fn format_amount(mut amount: Amount) -> String {
    amount.rescale(4);
    format!("{:.4}", amount)
}

pub struct StdoutProducer;

impl StdoutProducer {
    pub fn new() -> Self {
        Self
    }

    pub fn write<'a>(&self, accounts: impl IntoIterator<Item = &'a Account>) -> Result<(), IoError> {
        self.write_to(io::stdout(), accounts)
    }

    pub fn write_to<'a, W: Write>(
        &self,
        writer: W,
        accounts: impl IntoIterator<Item = &'a Account>,
    ) -> Result<(), IoError> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(writer);
        writer.write_record(["client", "available", "held", "total", "locked"])?;
        for account in accounts {
            writer.serialize(OutputRow::from(account))?;
        }
        writer.flush()?;
        Ok(())
    }
}

impl Default for StdoutProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde::Deserialize;

    use super::*;
    use crate::test_support::dec;

    #[derive(Debug, Deserialize)]
    struct SnapshotRow {
        client: ClientId,
        available: String,
        held: String,
        total: String,
        locked: bool,
    }

    #[test]
    fn brief_sample_snapshots_write_two_rows_in_any_order() {
        let accounts = [
            Account {
                client: 1,
                available: dec("1.5"),
                held: dec("0"),
                locked: false,
            },
            Account {
                client: 2,
                available: dec("2.0"),
                held: dec("0"),
                locked: false,
            },
        ];

        let mut buf = Vec::new();
        StdoutProducer::new()
            .write_to(&mut buf, accounts.iter())
            .expect("write succeeds");

        let mut reader = csv::Reader::from_reader(buf.as_slice());
        let mut got = HashSet::new();
        for row in reader.deserialize::<SnapshotRow>() {
            let row = row.expect("output row parses");
            got.insert((
                row.client,
                dec(&row.available),
                dec(&row.held),
                dec(&row.total),
                row.locked,
                row.available.clone(),
                row.held.clone(),
                row.total.clone(),
            ));
        }

        let expected = HashSet::from([
            (
                1,
                dec("1.5"),
                dec("0"),
                dec("1.5"),
                false,
                "1.5000".to_string(),
                "0.0000".to_string(),
                "1.5000".to_string(),
            ),
            (
                2,
                dec("2.0"),
                dec("0"),
                dec("2.0"),
                false,
                "2.0000".to_string(),
                "0.0000".to_string(),
                "2.0000".to_string(),
            ),
        ]);
        assert_eq!(got, expected);
    }

    #[test]
    fn empty_account_list_writes_header_only() {
        let mut buf = Vec::new();
        StdoutProducer::new()
            .write_to(&mut buf, std::iter::empty())
            .expect("write succeeds");
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "client,available,held,total,locked\n"
        );
    }

    #[test]
    fn locked_true_and_zero_balances_serialize() {
        let accounts = [Account {
            client: 1,
            available: dec("0"),
            held: dec("0"),
            locked: true,
        }];
        let mut buf = Vec::new();
        StdoutProducer::new()
            .write_to(&mut buf, accounts.iter())
            .expect("write succeeds");
        let stdout = String::from_utf8(buf).unwrap();
        assert!(stdout.contains("client,available,held,total,locked"));
        assert!(stdout.contains("true"));
        let mut reader = csv::Reader::from_reader(stdout.as_bytes());
        let row: SnapshotRow = reader.deserialize().next().unwrap().unwrap();
        assert_eq!(row.client, 1);
        assert_eq!(dec(&row.available), dec("0"));
        assert_eq!(dec(&row.held), dec("0"));
        assert_eq!(dec(&row.total), dec("0"));
        assert!(row.locked);
        assert_eq!(row.available, "0.0000");
        assert_eq!(row.held, "0.0000");
        assert_eq!(row.total, "0.0000");
    }
}
