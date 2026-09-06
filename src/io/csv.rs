use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;

use crate::types::{Amount, Event};

#[derive(Debug, Deserialize)]
struct RawRow {
    #[serde(rename = "type")]
    kind: String,
    client: String,
    tx: String,
    #[serde(default)]
    amount: Option<String>,
}

impl RawRow {
    fn into_event(self) -> Option<Event> {
        let client = parse_id(&self.client)?;
        let tx = parse_id(&self.tx)?;
        match self.kind.as_str() {
            "deposit" => Some(Event::Deposit {
                client,
                tx,
                amount: parse_required_amount(self.amount)?,
            }),
            "withdrawal" => Some(Event::Withdrawal {
                client,
                tx,
                amount: parse_required_amount(self.amount)?,
            }),
            "dispute" => Some(Event::Dispute { client, tx }),
            "resolve" => Some(Event::Resolve { client, tx }),
            "chargeback" => Some(Event::Chargeback { client, tx }),
            _ => None,
        }
    }
}

fn parse_id<T: FromStr>(raw: &str) -> Option<T> {
    raw.parse().ok()
}

fn parse_required_amount(amount: Option<String>) -> Option<Amount> {
    let raw = amount.filter(|s| !s.is_empty())?;
    let mut amount = Amount::from_str(&raw).ok()?;
    amount.rescale(4);
    Some(amount)
}

fn events_from_reader<R: Read>(reader: R) -> impl Iterator<Item = Event> {
    let reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .has_headers(true)
        .from_reader(reader);
    reader
        .into_deserialize::<RawRow>()
        .filter_map(|row| row.ok().and_then(RawRow::into_event))
}

pub struct CSVConsumer {
    path: PathBuf,
}

impl CSVConsumer {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn iter(&self) -> anyhow::Result<impl Iterator<Item = Event>> {
        let file = File::open(&self.path)?;
        Ok(events_from_reader(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Amount {
        let mut amount = Amount::from_str(s).expect("valid test amount");
        amount.rescale(4);
        amount
    }

    fn parse_csv(input: &str) -> Vec<Event> {
        events_from_reader(input.as_bytes()).collect()
    }

    #[test]
    fn sample_csv_yields_five_events_in_file_order() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test_data/sample.csv");
        let events: Vec<Event> = CSVConsumer::new(path)
            .iter()
            .expect("sample.csv opens")
            .collect();

        assert_eq!(
            events,
            vec![
                Event::Deposit {
                    client: 1,
                    tx: 1,
                    amount: dec("1.0"),
                },
                Event::Deposit {
                    client: 2,
                    tx: 2,
                    amount: dec("2.0"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 3,
                    amount: dec("2.0"),
                },
                Event::Withdrawal {
                    client: 1,
                    tx: 4,
                    amount: dec("1.5"),
                },
                Event::Withdrawal {
                    client: 2,
                    tx: 5,
                    amount: dec("3.0"),
                },
            ]
        );
    }

    #[test]
    fn whitespace_around_fields_is_trimmed() {
        let events = parse_csv(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             withdrawal,  2,  2,  2.0000\n",
        );

        assert_eq!(
            events,
            vec![
                Event::Deposit {
                    client: 1,
                    tx: 1,
                    amount: dec("1.0"),
                },
                Event::Withdrawal {
                    client: 2,
                    tx: 2,
                    amount: dec("2.0000"),
                },
            ]
        );
    }

    #[test]
    fn brief_sample_with_spaces_after_commas_parses() {
        let events = parse_csv(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             deposit, 2, 2, 2.0\n\
             deposit, 1, 3, 2.0\n\
             withdrawal, 1, 4, 1.5\n\
             withdrawal, 2, 5, 3.0\n",
        );
        assert_eq!(events.len(), 5);
        assert_eq!(
            events[0],
            Event::Deposit {
                client: 1,
                tx: 1,
                amount: dec("1.0"),
            }
        );
        assert_eq!(
            events[4],
            Event::Withdrawal {
                client: 2,
                tx: 5,
                amount: dec("3.0"),
            }
        );
    }

    #[test]
    fn dispute_resolve_chargeback_accept_three_fields_without_amount() {
        let events = parse_csv(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             dispute, 1, 1\n\
             resolve, 1, 1\n\
             chargeback, 1, 1\n",
        );
        assert_eq!(
            events,
            vec![
                Event::Deposit {
                    client: 1,
                    tx: 1,
                    amount: dec("1.0"),
                },
                Event::Dispute { client: 1, tx: 1 },
                Event::Resolve { client: 1, tx: 1 },
                Event::Chargeback { client: 1, tx: 1 },
            ]
        );
    }

    #[test]
    fn amount_on_dispute_resolve_chargeback_is_ignored() {
        let events = parse_csv(
            "type,client,tx,amount\n\
             dispute,1,1,99.0\n\
             resolve,1,1,99.0\n\
             chargeback,1,1,99.0\n",
        );
        assert_eq!(
            events,
            vec![
                Event::Dispute { client: 1, tx: 1 },
                Event::Resolve { client: 1, tx: 1 },
                Event::Chargeback { client: 1, tx: 1 },
            ]
        );
    }

    #[test]
    fn type_is_case_sensitive() {
        let events = parse_csv(
            "type,client,tx,amount\n\
             Deposit,1,1,1.0\n\
             WITHDRAWAL,1,2,1.0\n\
             Dispute,1,1,\n",
        );
        assert!(events.is_empty());
    }

    #[test]
    fn client_and_tx_id_bounds() {
        let ok = parse_csv(&format!(
            "type,client,tx,amount\ndeposit,{},{},1.0\n",
            u16::MAX,
            u32::MAX
        ));
        assert_eq!(
            ok,
            vec![Event::Deposit {
                client: u16::MAX,
                tx: u32::MAX,
                amount: dec("1.0"),
            }]
        );

        assert!(parse_csv("type,client,tx,amount\ndeposit,65536,1,1.0\n").is_empty());
        assert!(parse_csv("type,client,tx,amount\ndeposit,1,4294967296,1.0\n").is_empty());
    }

    #[test]
    fn extra_decimal_places_rescale_to_four() {
        let events = parse_csv("type,client,tx,amount\ndeposit,1,1,1.23456\n");
        match events.as_slice() {
            [Event::Deposit { amount, .. }] => assert_eq!(*amount, dec("1.2346")),
            other => panic!("expected one deposit, got {other:?}"),
        }
    }

    #[test]
    fn amounts_with_varying_precision_parse_as_one() {
        let events = parse_csv(
            "type,client,tx,amount\n\
             deposit,1,1,1\n\
             deposit,1,2,1.0\n\
             deposit,1,3,1.0000\n",
        );

        let expected = dec("1.0000");
        assert_eq!(events.len(), 3);
        for event in events {
            match event {
                Event::Deposit { amount, .. } => assert_eq!(amount, expected),
                other => panic!("expected deposit, got {other:?}"),
            }
        }
    }

    #[test]
    fn skippable_row_errors_do_not_abort_later_rows() {
        let events = parse_csv(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             transfer,1,2,1.0\n\
             deposit,1,3,2.0\n\
             deposit,1,4,\n\
             deposit,1,5,3.0\n\
             deposit,abc,6,1.0\n\
             deposit,1,7,1.0000\n\
             deposit,1,xyz,1.0\n\
             deposit,1,8,1\n\
             deposit,1,9,not-a-number\n\
             deposit,1,10,1.0\n",
        );

        assert_eq!(
            events,
            vec![
                Event::Deposit {
                    client: 1,
                    tx: 1,
                    amount: dec("1.0"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 3,
                    amount: dec("2.0"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 5,
                    amount: dec("3.0"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 7,
                    amount: dec("1.0000"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 8,
                    amount: dec("1"),
                },
                Event::Deposit {
                    client: 1,
                    tx: 10,
                    amount: dec("1.0"),
                },
            ]
        );
    }
}
