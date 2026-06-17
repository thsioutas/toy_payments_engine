# Toy Payments Engine

Implements a simple toy payments engine that reads a series of transactions
from a CSV, updates client accounts, handles disputes and chargebacks, and then
outputs the state of clients accounts as a CSV.

## Run instructions
You should be able to run the payments engine with:

```bash
cargo run -- transactions.csv > accounts.csv
```

## Assumptions

The following assumptions are made:

1. Only deposits can be disputed
2. Only a deposit transaction can register a new client account
3. Malformed CSV input files should not panic the application
4. Invalid or unexpected transactions (e.g. double disputes, withdrawals with insufficient funds,
  disputes on unknown tx IDs) should be silently ignored
5. Available balance can go negative: if funds from a disputed deposit were already partially withdrawn,
  `available` can go negative during the dispute
6. As per spec I assume that tx ids are unique and I don't check for duplicates.

## Future work

1. Use `tracing` instead of eprintln
2. Spec says to silently ignore invalid business cases. In a real system I would use tracing with structured spans
3. Better CSV validation and error propagation (header checking)
4. Deposits storage. Most probably a more efficient way of checking deposits (available for dispute)
  can be found. Right now this grows unboundedly.

## Error handling

The application fails and terminates only in the following cases:

1. the necessary input file is not given
2. it cannot open the given input file
3. writing output to stdout fails (serialization or flush errors)

Other issues are either logged or silently ignored.
For example:

1. Malformed CSV headers will produce relevant log errors.
  We could check the headers of the file but we consider it outside of the scope of this assignment
2. Malformed CSV rows (i.e. deposit tx without amount) are logged
3. Invalid or unexpected business transactions are silently ignored

## Testing

Run the full test suite with:

```bash
cargo test
```

### Unit tests

- **`src/reader.rs`** — unit tests for CSV parsing
- **`src/engine.rs`** — unit tests for account logic

### Integration tests

- **`tests/e2e.rs`** — integration e2e tests (`CsvReader` → `Engine` → `output_records`) with raw CSV bytes

## AI assistance

Claude agent has been used for:

1. UTs and integration tests (even though some edge cases were missed and guidance was needed)
2. Repetitive work (i.e. `impl TryFrom<CsvRecord> for EngineRecord`, etc.)
3. Improve docs (cargo and readme)

Example prompts used during development:

- "Uts for `engine.rs`"
- "I want a UT that I'm able to dispute another deposit on an already locked account and a UT that I cannot dispute a withdraw"
- "Add some e2e tests under `tests/e2e.rs`"
- "`@types.rs#L1:38` add docs"
- "Improve README.md"
