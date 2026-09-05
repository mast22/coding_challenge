use std::path::PathBuf;
use std::str::FromStr;

use crate::types::Amount;

pub(crate) fn dec(s: &str) -> Amount {
    let mut amount = Amount::from_str(s).expect("valid test amount");
    amount.rescale(4);
    amount
}

pub(crate) fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join(name)
}
