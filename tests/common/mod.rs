use std::path::PathBuf;
use std::str::FromStr;

use rust_decimal::Decimal;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join(name)
}

pub fn dec(s: &str) -> Decimal {
    let mut amount = Decimal::from_str(s).expect("valid test amount");
    amount.rescale(4);
    amount
}
