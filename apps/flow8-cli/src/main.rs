use clap::{Parser, Subcommand};
use flow8_model::{InputId, MixDestination};
use flow8_protocol::{TxCommand, encode};

#[derive(Parser)]
#[command(name = "flow8-cli", about = "FLOW 8 PC Controller protocol utility")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    EncodeGain {
        #[arg(long, default_value_t = 0)]
        input: u8,
        #[arg(long)]
        db: f32,
    },
    EncodeRoute {
        #[arg(long, default_value_t = 0)]
        input: u8,
        #[arg(long, default_value = "main")]
        destination: String,
        #[arg(long)]
        normalized: f32,
    },
}

fn input(value: u8) -> InputId {
    InputId::ALL[value.min(6) as usize]
}
fn destination(value: &str) -> MixDestination {
    match value.to_ascii_lowercase().as_str() {
        "mon1" => MixDestination::Monitor1,
        "mon2" => MixDestination::Monitor2,
        "fx1" => MixDestination::Fx1,
        "fx2" => MixDestination::Fx2,
        _ => MixDestination::Main,
    }
}
fn print_hex(bytes: &[u8]) {
    println!(
        "{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

fn main() {
    let cli = Cli::parse();
    let command = match cli.command {
        Command::EncodeGain { input: id, db } => TxCommand::Gain {
            input: input(id),
            db,
        },
        Command::EncodeRoute {
            input: id,
            destination: target,
            normalized,
        } => TxCommand::RouteLevel {
            source: input(id),
            destination: destination(&target),
            normalized,
        },
    };
    match encode(&command) {
        Ok(bytes) => print_hex(&bytes),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
