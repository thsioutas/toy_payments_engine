use rust_decimal::Decimal;
use serde::Serialize;

/// A validated transaction record ready for the engine to process.
///
/// Produced by [`crate::reader::Reader`] implementations after parsing and
/// validation. Each variant carries only the fields it requires — deposit and
/// withdrawal amounts are guaranteed to be present, dispute/resolve/chargeback
/// reference a prior deposit by `tx` ID.
#[cfg_attr(test, derive(Debug))]
pub enum EngineRecord {
    /// Credit to a client account. Increases available and total funds.
    Deposit {
        client: u16,
        tx: u32,
        amount: Decimal,
    },
    /// Debit from a client account. Decreases available and total funds.
    Withdrawal {
        client: u16,
        tx: u32,
        amount: Decimal,
    },
    /// Claim that a deposit was erroneous. Moves funds from available to held.
    Dispute { client: u16, tx: u32 },
    /// Resolution of a dispute. Returns held funds to available.
    Resolve { client: u16, tx: u32 },
    /// Final reversal of a disputed deposit. Removes held funds and locks the account.
    Chargeback { client: u16, tx: u32 },
}

/// The final state of a client account.
#[derive(Serialize)]
pub struct OutputRecord {
    pub client: u16,
    /// Funds available for trading or withdrawal (`total - held`).
    pub available: Decimal,
    /// Funds held pending dispute resolution (`total - available`).
    pub held: Decimal,
    /// Total funds (`available + held`).
    pub total: Decimal,
    /// `true` if a chargeback has occurred; deposits and withdrawals are rejected.
    pub locked: bool,
}
