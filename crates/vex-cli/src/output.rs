use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    Text,
}

pub fn print<T: Serialize + std::fmt::Debug>(value: &T, format: Format) {
    match format {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(value).unwrap());
        }
        Format::Text => {
            println!("{value:#?}");
        }
    }
}
