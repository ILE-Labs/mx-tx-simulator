mod errors;
mod formatter;
mod response;
mod state;

use clap::{Parser, Subcommand};
use errors::SimulationError;
use formatter::format_result;
use multiversx_sc_scenario::{
    scenario::ScenarioRunner,
    scenario_model::{ScCallStep, TxExpect},
    ScenarioWorld,
};
use response::{StateSnapshot, TransactionResult};
use state::StateConfig;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Simulate {
        #[arg(short, long)]
        contract: String,

        #[arg(short, long)]
        function: String,

        #[arg(short = 'u', long)]
        caller: String,

        #[arg(short, long, default_value_t = 10_000_000)]
        gas_limit: u64,

        #[arg(short, long, default_value = "state.json")]
        state_file: String,

        #[arg(short, long, value_delimiter = ',')]
        args: Option<Vec<String>>,

        #[arg(long, default_value = "sc:target_contract")]
        target_address: String,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run_simulation(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_simulation(cli: Cli) -> Result<(), SimulationError> {
    match &cli.command {
        Commands::Simulate {
            contract,
            function,
            caller,
            gas_limit,
            state_file,
            args,
            target_address,
        } => {
            println!("Booting MultiversX Local Simulator...");
            println!("Loading state from: {}", state_file);

            // Initialize ScenarioWorld
            let mut world = ScenarioWorld::new();
            world.register_contract(
                format!("file:{}", contract).as_str(),
                counter::ContractBuilder,
            );

            // Load and apply state from JSON file
            let state_config = StateConfig::from_file(state_file)?;
            state_config.apply_to_world(&mut world)?;

            println!("Impersonating caller: {} (Signature bypassed)", caller);
            println!("Executing: {} -> {}()", contract, function);

            // Capture state BEFORE execution
            // Note: For POC, we'll extract storage manually from the state file
            // In production, we'd query the ScenarioWorld state directly
            let before_state = StateSnapshot::empty();

            // Build transaction
            let mut tx = ScCallStep::new()
                .from(caller.as_str())
                .to(target_address.as_str())
                .function(function.as_str())
                .gas_limit(*gas_limit);

            // Add arguments if provided
            if let Some(arg_list) = args {
                for (i, arg) in arg_list.iter().enumerate() {
                    let validated_arg = validate_hex_arg(arg, i)?;
                    tx = tx.argument(validated_arg.as_str());
                }
            }

            // Don't set expectations - let the transaction execute and capture results
            tx = tx.no_expect();

            // Execute transaction
            world.run_sc_call_step(&mut tx);

            // Debug: Print response details
            if let Some(ref response) = tx.response {
                eprintln!("DEBUG: Gas used: {}", response.gas_used);
                eprintln!("DEBUG: Status: {:?}", response.tx_error.status);
                eprintln!("DEBUG: Message: {}", response.tx_error.message);
            } else {
                eprintln!("DEBUG: No response!");
            }

            // Capture state AFTER execution
            // Note: For POC, we extract from response
            let after_state = StateSnapshot::empty();

            // Extract transaction result
            let result = TransactionResult::from_response(&tx, &before_state, &after_state, *gas_limit);

            // Format and print output
            let output = format_result(&result, target_address);
            println!("\nSimulation Result:\n-----------------");
            println!("{}", serde_json::to_string_pretty(&output).unwrap());

            Ok(())
        }
    }
}

/// Validate and normalize hex argument
fn validate_hex_arg(arg: &str, index: usize) -> Result<String, SimulationError> {
    let trimmed = arg.trim();

    // Check if it starts with 0x
    let hex_str = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        &trimmed[2..]
    } else {
        trimmed
    };

    // Validate hex characters
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(SimulationError::InvalidArgument {
            index,
            reason: format!("'{}' contains non-hex characters", arg),
        });
    }

    // Return with 0x prefix
    Ok(format!("0x{}", hex_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hex_arg() {
        assert_eq!(validate_hex_arg("0x1234", 0).unwrap(), "0x1234");
        assert_eq!(validate_hex_arg("1234", 0).unwrap(), "0x1234");
        assert_eq!(validate_hex_arg("0X1234", 0).unwrap(), "0x1234");
        assert!(validate_hex_arg("xyz", 0).is_err());
        assert!(validate_hex_arg("12 34", 0).is_err());
    }
}
