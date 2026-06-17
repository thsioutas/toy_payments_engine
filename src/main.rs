use anyhow::{Context, Result, anyhow};
use std::env;
use std::fs::File;
use std::io;

use toy_payments_engine::{engine::Engine, reader::CsvReader};

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: toy_payments_engine <transactions.csv>"))?;
    let file = File::open(&path).with_context(|| format!("failed to open {path}"))?;

    let mut engine = Engine::new();
    engine.process(&mut CsvReader::new(file));

    let mut writer = csv::Writer::from_writer(io::stdout());
    for record in engine.output_records() {
        writer.serialize(record)?;
    }
    writer.flush()?;

    Ok(())
}
