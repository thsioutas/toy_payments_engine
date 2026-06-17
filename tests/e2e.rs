use std::io::Cursor;

use rust_decimal_macros::dec;
use toy_payments_engine::{engine::Engine, reader::CsvReader, types::OutputRecord};

fn run(input: &'static [u8]) -> Vec<OutputRecord> {
    let mut engine = Engine::new();
    engine.process(&mut CsvReader::new(Cursor::new(input)));
    let mut out: Vec<_> = engine.output_records().collect();
    out.sort_by_key(|r| r.client);
    out
}

#[test]
fn spec_example() {
    let out = run(b"type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0");

    assert_eq!(out.len(), 2);

    assert_eq!(out[0].client, 1);
    assert_eq!(out[0].available, dec!(1.5));
    assert_eq!(out[0].held, dec!(0));
    assert_eq!(out[0].total, dec!(1.5));
    assert!(!out[0].locked);

    assert_eq!(out[1].client, 2);
    assert_eq!(out[1].available, dec!(2.0));
    assert_eq!(out[1].held, dec!(0));
    assert_eq!(out[1].total, dec!(2.0));
    assert!(!out[1].locked);
}

#[test]
fn dispute_and_resolve_restores_funds() {
    let out = run(b"type,client,tx,amount
deposit,1,1,10.0
dispute,1,1,
resolve,1,1,");

    assert_eq!(out[0].available, dec!(10.0));
    assert_eq!(out[0].held, dec!(0));
    assert_eq!(out[0].total, dec!(10.0));
    assert!(!out[0].locked);
}

#[test]
fn dispute_and_chargeback_locks_account() {
    let out = run(b"type,client,tx,amount
deposit,1,1,10.0
deposit,1,2,5.0
dispute,1,1
chargeback,1,1");

    assert_eq!(out[0].available, dec!(5.0));
    assert_eq!(out[0].held, dec!(0));
    assert_eq!(out[0].total, dec!(5.0));
    assert!(out[0].locked);
}

#[test]
fn locked_account_ignores_new_deposits_and_withdrawals() {
    let out = run(b"type,client,tx,amount
deposit,1,1,10.0
dispute,1,1
chargeback,1,1
deposit,1,2,99.0
withdrawal,1,3,1.0");

    assert_eq!(out[0].available, dec!(0));
    assert_eq!(out[0].total, dec!(0));
    assert!(out[0].locked);
}

#[test]
fn locked_account_can_still_dispute_remaining_deposits() {
    let out = run(b"type,client,tx,amount
deposit,1,1,10.0
deposit,1,2,5.0
dispute,1,1,
chargeback,1,1,
dispute,1,2,");

    assert!(out[0].locked);
    assert_eq!(out[0].available, dec!(0));
    assert_eq!(out[0].held, dec!(5.0));
    assert_eq!(out[0].total, dec!(5.0));
}

#[test]
fn withdrawal_with_insufficient_funds_is_rejected() {
    let out = run(b"type,client,tx,amount
deposit,1,1,5.0
withdrawal,1,2,10.0");

    assert_eq!(out[0].available, dec!(5.0));
    assert_eq!(out[0].total, dec!(5.0));
}

#[test]
fn handles_whitespace_and_four_decimal_places() {
    let out = run(b"type, client, tx, amount
deposit, 1, 1, 1.5000
withdrawal, 1, 2, 0.5000");

    assert_eq!(out[0].available, dec!(1.0));
}

#[test]
fn multiple_clients_are_processed_independently() {
    let out = run(b"type,client,tx,amount
deposit,1,1,10.0
deposit,2,2,20.0
withdrawal,1,3,3.0
dispute,2,2,");

    assert_eq!(out[0].client, 1);
    assert_eq!(out[0].available, dec!(7.0));
    assert_eq!(out[0].held, dec!(0));

    assert_eq!(out[1].client, 2);
    assert_eq!(out[1].available, dec!(0));
    assert_eq!(out[1].held, dec!(20.0));
    assert_eq!(out[1].total, dec!(20.0));
}
