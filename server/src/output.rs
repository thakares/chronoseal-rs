use crate::cli::OutputFormat;
use serde::Serialize;

pub fn print<T>(format: OutputFormat, value: &T) -> Result<(), Box<dyn std::error::Error>>
where
    T: Serialize + TextOutput,
{
    match format {
        OutputFormat::Text => println!("{}", value.to_text()),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(value)?),
    }
    Ok(())
}

pub trait TextOutput {
    fn to_text(&self) -> String;
}
