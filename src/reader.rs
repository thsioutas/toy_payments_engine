use anyhow::Result;
use csv::ReaderBuilder;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::io::Read;

use crate::types::EngineRecord;

/// Source of transaction records for the payments engine.
/// Returns `None` when the source is exhausted.
pub trait Reader {
    fn next_record(&mut self) -> Option<EngineRecord>;
}

/// The type of the CSV record
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// The raw CSV record
/// This is different than the `EngineRecord` because we cannot deserialize the necessary format
#[derive(Deserialize)]
struct CsvRecord {
    #[serde(rename = "type")]
    tx_type: TxType,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

impl TryFrom<CsvRecord> for EngineRecord {
    type Error = String;
    fn try_from(r: CsvRecord) -> Result<Self, Self::Error> {
        match r.tx_type {
            TxType::Deposit => Ok(Self::Deposit {
                client: r.client,
                tx: r.tx,
                amount: r
                    .amount
                    .ok_or(format!("Deposit transaction {} without amount", r.tx))?,
            }),
            TxType::Withdrawal => Ok(Self::Withdrawal {
                client: r.client,
                tx: r.tx,
                amount: r
                    .amount
                    .ok_or(format!("Withdrawal transaction {} without amount", r.tx))?,
            }),
            TxType::Dispute => Ok(Self::Dispute {
                client: r.client,
                tx: r.tx,
            }),
            TxType::Resolve => Ok(Self::Resolve {
                client: r.client,
                tx: r.tx,
            }),
            TxType::Chargeback => Ok(Self::Chargeback {
                client: r.client,
                tx: r.tx,
            }),
        }
    }
}

pub struct CsvReader<R: Read> {
    iter: csv::DeserializeRecordsIntoIter<R, CsvRecord>,
}

impl<R: Read> CsvReader<R> {
    pub fn new(reader: R) -> Self {
        let csv_reader = ReaderBuilder::new()
            .trim(csv::Trim::All)
            // Allow rows with no amount field (dispute, resolve, chargeback)
            .flexible(true)
            .from_reader(reader);
        Self {
            iter: csv_reader.into_deserialize(),
        }
    }
}

impl<R: Read> Reader for CsvReader<R> {
    fn next_record(&mut self) -> Option<EngineRecord> {
        loop {
            match self.iter.next()? {
                Err(e) => eprintln!("skipping malformed row: {e}"),
                Ok(raw) => match EngineRecord::try_from(raw) {
                    Err(e) => eprintln!("skipping malformed row: {e}"),
                    Ok(record) => return Some(record),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use std::io::Cursor;

    use super::*;
    use crate::types::EngineRecord;

    fn reader(input: &'static [u8]) -> CsvReader<Cursor<&'static [u8]>> {
        CsvReader::new(Cursor::new(input))
    }

    #[test]
    fn parses_deposit() {
        let mut r = reader(b"type,client,tx,amount\ndeposit,1,1,1.5\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(
                record,
                EngineRecord::Deposit {
                    client: 1,
                    tx: 1,
                    ..
                }
            ),
            "{record:?}"
        );
    }

    #[test]
    fn parses_withdrawal() {
        let mut r = reader(b"type,client,tx,amount\nwithdrawal,2,2,1.0\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(
                record,
                EngineRecord::Withdrawal {
                    client: 2,
                    tx: 2,
                    ..
                }
            ),
            "{record:?}"
        );
    }

    #[test]
    fn parses_dispute_without_amount() {
        let mut r = reader(b"type,client,tx,amount\ndispute,1,1\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(record, EngineRecord::Dispute { client: 1, tx: 1 }),
            "{record:?}"
        );
    }

    #[test]
    fn parses_resolve_without_amount() {
        let mut r = reader(b"type,client,tx,amount\nresolve,1,1\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(record, EngineRecord::Resolve { client: 1, tx: 1 }),
            "{record:?}"
        );
    }

    #[test]
    fn parses_chargeback_without_amount() {
        let mut r = reader(b"type,client,tx,amount\nchargeback,1,1\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(record, EngineRecord::Chargeback { client: 1, tx: 1 }),
            "{record:?}"
        );
    }

    #[test]
    fn parses_amount_with_four_decimal_places() {
        let mut r = reader(b"type,client,tx,amount\ndeposit,1,1,1.2345\n");
        let record = r.next_record().expect("expected a record");
        if let EngineRecord::Deposit { amount, .. } = record {
            assert_eq!(amount, "1.2345".parse::<Decimal>().unwrap());
        } else {
            panic!("expected deposit, got {record:?}");
        }
    }

    #[test]
    fn trims_whitespace() {
        let mut r = reader(b"type, client, tx, amount\n deposit , 1 , 1 , 1.5 \n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(
                record,
                EngineRecord::Deposit {
                    client: 1,
                    tx: 1,
                    ..
                }
            ),
            "{record:?}"
        );
    }

    #[test]
    fn skips_deposit_missing_amount_and_returns_next() {
        let mut r = reader(b"type,client,tx,amount\ndeposit,1,1\nwithdrawal,2,2,1.0\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(
                record,
                EngineRecord::Withdrawal {
                    client: 2,
                    tx: 2,
                    ..
                }
            ),
            "{record:?}"
        );
    }

    #[test]
    fn skips_malformed_row_and_returns_next() {
        let mut r = reader(b"type,client,tx,amount\nbadtype,1,1,1.0\ndeposit,1,2,1.0\n");
        let record = r.next_record().expect("expected a record");
        assert!(
            matches!(
                record,
                EngineRecord::Deposit {
                    client: 1,
                    tx: 2,
                    ..
                }
            ),
            "{record:?}"
        );
    }

    #[test]
    fn returns_none_on_empty_input() {
        let mut r = reader(b"type,client,tx,amount\n");
        assert!(r.next_record().is_none());
    }

    #[test]
    fn returns_none_after_last_record() {
        let mut r = reader(b"type,client,tx,amount\ndeposit,1,1,1.0\n");
        r.next_record();
        assert!(r.next_record().is_none());
    }
}
