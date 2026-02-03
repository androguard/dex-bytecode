
use clap::Parser;
use log::{error, info, warn, LevelFilter};
use simple_logger::SimpleLogger;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// input bytecode to disassemble
    #[arg(short, long)]
    input: String,

    /// enable debug logs
    #[arg(short, long)]
    debug: bool,
}

fn main() {
    let args = Args::parse();

    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

}