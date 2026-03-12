mod errors;
mod formatter;
mod gas;
mod response;
mod state;

use clap::{Parser, Subcommand};
use errors::SimulationError;
use formatter::format_result;
use gas::GasEstimator;
use multiversx_sc_scenario::{
    scenario::run_vm::ExecutorConfig, scenario::ScenarioRunner, scenario_model::ScCallStep,
    ScenarioWorld,
};
use response::{StateSnapshot, TransactionResult};
use state::StateConfig;
use std::panic;
use std::time::Instant;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single transaction simulation
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
    /// Run the full demo showcasing all POC features
    Demo,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Simulate { .. } => {
            if let Err(e) = run_simulate(&cli.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Demo => run_demo(),
    }
}

/// Shared simulation logic used by both `simulate` and `demo`
fn execute_simulation(
    contract: &str,
    function: &str,
    caller: &str,
    gas_limit: u64,
    state_file: &str,
    args: &[String],
    target_address: &str,
) -> Result<serde_json::Value, SimulationError> {
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::Experimental);
    world.register_contract(
        format!("file:{}", contract).as_str(),
        counter::ContractBuilder,
    );

    let state_config = StateConfig::from_file(state_file)?;
    state_config.apply_to_world(&mut world)?;

    let before_state = StateSnapshot::empty();

    let mut tx = ScCallStep::new()
        .from(caller)
        .to(target_address)
        .function(function)
        .gas_limit(gas_limit);

    for (i, arg) in args.iter().enumerate() {
        let validated_arg = validate_hex_arg(arg, i)?;
        tx = tx.argument(validated_arg.as_str());
    }

    tx = tx.no_expect();
    world.run_sc_call_step(&mut tx);

    let after_state = StateSnapshot::empty();
    let result = TransactionResult::from_response(&tx, &before_state, &after_state, gas_limit);

    let estimator = GasEstimator::new();
    let gas_estimate = estimator.estimate(result.gas_used, function, function.len());
    let output = format_result(&result, target_address, &gas_estimate);
    Ok(output)
}

fn run_simulate(command: &Commands) -> Result<(), SimulationError> {
    match command {
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
            println!("Impersonating caller: {} (Signature bypassed)", caller);
            println!("Executing: {} -> {}()", contract, function);

            let output = execute_simulation(
                contract,
                function,
                caller,
                *gas_limit,
                state_file,
                args.as_deref().unwrap_or(&[]),
                target_address,
            )?;

            println!("\nSimulation Result:\n-----------------");
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── F5: Demo Scenario ──────────────────────────────────────────────────

const DEMO_CONTRACT: &str = "counter/output/counter.wasm";
const DEMO_STATE: &str = "examples/counter_initial.json";
const DEMO_STATE_MISSING: &str = "examples/counter_missing_account.json";
const DEMO_CALLER: &str = "address:wallet1";
const DEMO_TARGET: &str = "sc:target_contract";
const DEMO_GAS: u64 = 10_000_000;

fn run_demo() {
    let demo_start = Instant::now();

    println!("==========================================================");
    println!("  MultiversX Local Transaction Simulator - POC Demo");
    println!("==========================================================");
    println!();
    println!("This demo showcases all POC features:");
    println!("  F1 - Transaction Simulation Engine");
    println!("  F4 - Gas Cost Prediction (GoVM-inspired)");
    println!("  Graceful error handling for edge cases");
    println!();

    // ── Scenario 1: View call ──
    print_scenario_header(1, "Read query (view function)", "get");
    run_demo_scenario(DEMO_CONTRACT, "get", DEMO_CALLER, DEMO_STATE);

    // ── Scenario 2: Write transaction ──
    print_scenario_header(2, "State-changing transaction", "increment");
    run_demo_scenario(DEMO_CONTRACT, "increment", DEMO_CALLER, DEMO_STATE);

    // ── Scenario 3: Missing account (F2 - DebuggerBackend panic fix) ──
    print_scenario_header(
        3,
        "Missing caller account (graceful error handling)",
        "increment",
    );
    println!("  State file has NO wallet account - previously caused a DebuggerBackend panic.");
    println!("  The simulator now catches this and reports it as an error.");
    println!();
    run_demo_scenario(
        DEMO_CONTRACT,
        "increment",
        "address:nonexistent_wallet",
        DEMO_STATE_MISSING,
    );

    // ── Scenario 4: Invalid function name ──
    print_scenario_header(4, "Calling a non-existent function", "does_not_exist");
    run_demo_scenario(DEMO_CONTRACT, "does_not_exist", DEMO_CALLER, DEMO_STATE);

    // ── Summary ──
    let elapsed = demo_start.elapsed();
    println!("==========================================================");
    println!("  Demo completed in {:.2}s", elapsed.as_secs_f64());
    println!("==========================================================");
}

/// Execute a demo scenario, catching any VM panics gracefully
fn run_demo_scenario(contract: &str, function: &str, caller: &str, state_file: &str) {
    // Suppress default panic output during catch_unwind
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let result = panic::catch_unwind(|| {
        execute_simulation(
            contract,
            function,
            caller,
            DEMO_GAS,
            state_file,
            &[],
            DEMO_TARGET,
        )
    });

    // Restore the default panic hook
    panic::set_hook(prev_hook);

    match result {
        Ok(Ok(output)) => print_scenario_result(&output),
        Ok(Err(e)) => print_scenario_error(&e),
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };
            println!("  [CAUGHT VM PANIC] {}", msg);
            println!("  This is the DebuggerBackend crash (Issue #1267) - now handled gracefully.");
            println!();
        }
    }
}

fn print_scenario_header(num: u8, description: &str, function: &str) {
    println!("----------------------------------------------------------");
    println!("  Scenario {}: {}", num, description);
    println!("  Function: {}() | Contract: {}", function, DEMO_CONTRACT);
    println!("----------------------------------------------------------");
}

fn print_scenario_result(output: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(output).unwrap());
    println!();
}

fn print_scenario_error(err: &SimulationError) {
    println!("  [ERROR] {}", err);
    println!();
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
