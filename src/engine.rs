use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::reader::Reader;
use crate::types::{EngineRecord, OutputRecord};

struct Deposit {
    amount: Decimal,
    disputed: bool,
}

struct Account {
    available: Decimal,
    held: Decimal,
    locked: bool,
    deposits: HashMap<u32, Deposit>,
}

impl Account {
    fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
            deposits: HashMap::new(),
        }
    }

    fn total(&self) -> Decimal {
        self.available + self.held
    }

    fn deposit(&mut self, tx: u32, amount: Decimal) {
        if self.locked {
            return;
        }
        self.available += amount;
        self.deposits.insert(
            tx,
            Deposit {
                amount,
                disputed: false,
            },
        );
    }

    fn withdraw(&mut self, amount: Decimal) {
        if self.locked || self.available < amount {
            return;
        }
        self.available -= amount;
    }

    fn dispute(&mut self, tx: u32) {
        let Some(deposit) = self.deposits.get_mut(&tx) else {
            return;
        };
        if deposit.disputed {
            return;
        }
        deposit.disputed = true;
        self.available -= deposit.amount;
        self.held += deposit.amount;
    }

    fn resolve(&mut self, tx: u32) {
        let Some(deposit) = self.deposits.get_mut(&tx) else {
            return;
        };
        if !deposit.disputed {
            return;
        }
        deposit.disputed = false;
        self.available += deposit.amount;
        self.held -= deposit.amount;
    }

    fn chargeback(&mut self, tx: u32) {
        let Some(deposit) = self.deposits.get(&tx) else {
            return;
        };
        if !deposit.disputed {
            return;
        }
        let amount = deposit.amount;
        self.deposits.remove(&tx);
        self.held -= amount;
        self.locked = true;
    }
}

/// Processes transaction records and maintains client account state.
///
/// Feed records via [`Engine::process`], then retrieve final balances with
/// [`Engine::output_records`].
pub struct Engine {
    accounts: HashMap<u16, Account>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Creates an engine with no accounts.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    /// Drains `reader` and applies each record to the relevant account.
    pub fn process(&mut self, reader: &mut impl Reader) {
        while let Some(record) = reader.next_record() {
            self.apply(record);
        }
    }

    fn apply(&mut self, record: EngineRecord) {
        match record {
            EngineRecord::Deposit { client, tx, amount } => {
                self.accounts
                    .entry(client)
                    .or_insert_with(Account::new)
                    .deposit(tx, amount);
            }
            EngineRecord::Withdrawal { client, amount, .. } => {
                if let Some(account) = self.accounts.get_mut(&client) {
                    account.withdraw(amount);
                }
            }
            EngineRecord::Dispute { client, tx } => {
                if let Some(account) = self.accounts.get_mut(&client) {
                    account.dispute(tx);
                }
            }
            EngineRecord::Resolve { client, tx } => {
                if let Some(account) = self.accounts.get_mut(&client) {
                    account.resolve(tx);
                }
            }
            EngineRecord::Chargeback { client, tx } => {
                if let Some(account) = self.accounts.get_mut(&client) {
                    account.chargeback(tx);
                }
            }
        }
    }

    /// Returns the final state of every account seen during processing.
    pub fn output_records(&self) -> impl Iterator<Item = OutputRecord> {
        self.accounts.iter().map(|(&client, account)| OutputRecord {
            client,
            available: account.available,
            held: account.held,
            total: account.total(),
            locked: account.locked,
        })
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::Engine;
    use crate::reader::Reader;
    use crate::types::EngineRecord;

    struct VecReader(std::vec::IntoIter<EngineRecord>);

    impl Reader for VecReader {
        fn next_record(&mut self) -> Option<EngineRecord> {
            self.0.next()
        }
    }

    fn run(records: Vec<EngineRecord>) -> Vec<crate::types::OutputRecord> {
        let mut engine = Engine::new();
        engine.process(&mut VecReader(records.into_iter()));
        let mut out: Vec<_> = engine.output_records().collect();
        out.sort_by_key(|r| r.client);
        out
    }

    #[test]
    fn deposit_increases_available() {
        let out = run(vec![EngineRecord::Deposit {
            client: 1,
            tx: 1,
            amount: dec!(10.0),
        }]);
        assert_eq!(out[0].available, dec!(10.0));
        assert_eq!(out[0].held, dec!(0));
        assert_eq!(out[0].total, dec!(10.0));
        assert!(!out[0].locked);
    }

    #[test]
    fn withdrawal_decreases_available() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec!(3.0),
            },
        ]);
        assert_eq!(out[0].available, dec!(7.0));
        assert_eq!(out[0].total, dec!(7.0));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_rejected() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(5.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec!(10.0),
            },
        ]);
        assert_eq!(out[0].available, dec!(5.0));
    }

    #[test]
    fn dispute_moves_funds_to_held_total_unchanged() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].available, dec!(0));
        assert_eq!(out[0].held, dec!(10.0));
        assert_eq!(out[0].total, dec!(10.0));
    }

    #[test]
    fn double_dispute_is_ignored() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Dispute { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].available, dec!(0));
        assert_eq!(out[0].held, dec!(10.0));
        assert_eq!(out[0].total, dec!(10.0));
    }

    #[test]
    fn dispute_on_nonexistent_tx_is_ignored() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 99 },
        ]);
        assert_eq!(out[0].available, dec!(10.0));
        assert_eq!(out[0].held, dec!(0));
    }

    #[test]
    fn resolve_returns_held_funds_to_available() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Resolve { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].available, dec!(10.0));
        assert_eq!(out[0].held, dec!(0));
        assert_eq!(out[0].total, dec!(10.0));
    }

    #[test]
    fn resolve_on_non_disputed_tx_is_ignored() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Resolve { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].available, dec!(10.0));
        assert_eq!(out[0].held, dec!(0));
    }

    #[test]
    fn chargeback_removes_held_and_locks_account() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Chargeback { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].held, dec!(0));
        assert_eq!(out[0].total, dec!(0));
        assert!(out[0].locked);
    }

    #[test]
    fn chargeback_cannot_happen_twice() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Chargeback { client: 1, tx: 1 },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Chargeback { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].held, dec!(0));
        assert_eq!(out[0].total, dec!(0));
    }

    #[test]
    fn locked_account_rejects_deposits_and_withdrawals() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Chargeback { client: 1, tx: 1 },
            EngineRecord::Deposit {
                client: 1,
                tx: 2,
                amount: dec!(100.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 3,
                amount: dec!(1.0),
            },
        ]);
        assert_eq!(out[0].available, dec!(0));
        assert_eq!(out[0].total, dec!(0));
    }

    #[test]
    fn multiple_clients_are_independent() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Deposit {
                client: 2,
                tx: 2,
                amount: dec!(20.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 3,
                amount: dec!(3.0),
            },
        ]);
        assert_eq!(out[0].client, 1);
        assert_eq!(out[0].available, dec!(7.0));
        assert_eq!(out[1].client, 2);
        assert_eq!(out[1].available, dec!(20.0));
    }

    #[test]
    fn dispute_on_another_clients_tx_is_ignored() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Dispute { client: 2, tx: 1 },
        ]);
        let client1 = out.iter().find(|r| r.client == 1).unwrap();
        assert_eq!(client1.available, dec!(10.0));
        assert_eq!(client1.held, dec!(0));
    }

    #[test]
    fn can_dispute_another_deposit_on_locked_account() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Deposit {
                client: 1,
                tx: 2,
                amount: dec!(5.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
            EngineRecord::Chargeback { client: 1, tx: 1 },
            // account is now locked, but tx 2 can still be disputed
            EngineRecord::Dispute { client: 1, tx: 2 },
        ]);
        assert!(out[0].locked);
        assert_eq!(out[0].available, dec!(0));
        assert_eq!(out[0].held, dec!(5.0));
        assert_eq!(out[0].total, dec!(5.0));
    }

    #[test]
    fn dispute_after_partial_withdrawal_makes_available_negative() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec!(5.0),
            },
            EngineRecord::Dispute { client: 1, tx: 1 },
        ]);
        assert_eq!(out[0].available, dec!(-5.0));
        assert_eq!(out[0].held, dec!(10.0));
        assert_eq!(out[0].total, dec!(5.0));
    }

    #[test]
    fn cannot_dispute_a_withdrawal() {
        let out = run(vec![
            EngineRecord::Deposit {
                client: 1,
                tx: 1,
                amount: dec!(10.0),
            },
            EngineRecord::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec!(3.0),
            },
            EngineRecord::Dispute { client: 1, tx: 2 },
        ]);
        // withdrawals are not stored, so dispute is ignored
        assert_eq!(out[0].available, dec!(7.0));
        assert_eq!(out[0].held, dec!(0));
    }
}
