use std::env;
use std::path::Path;

use rust_challenge::{CSVConsumer, Engine, StdoutProducer};

fn main() -> anyhow::Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("missing path to transactions CSV"))?;
    run(path)
}

fn run(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let mut engine = Engine::new();
    let consumer = CSVConsumer::new(path);
    for event in consumer.iter()? {
        engine.apply(event);
    }
    StdoutProducer::new().write(engine.accounts())
}
