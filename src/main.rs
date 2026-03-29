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
    /// Run the counter scenario test
    Demo,
    /// Run the piggybank scenario test (complex contract)
    DemoPiggybank,
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
        Commands::DemoPiggybank => run_demo_piggybank(),
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
    world.register_contract(
        "file:piggybank/output/piggybank.wasm",
        piggybank::ContractBuilder,
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

// ── F5: Real Scenario Test ─────────────────────────────────────────────

const DEMO_CONTRACT: &str = "counter/output/counter.wasm";
const DEMO_STATE: &str = "examples/counter_initial.json";
const DEMO_CALLER: &str = "address:wallet1";
const DEMO_TARGET: &str = "sc:target_contract";
const DEMO_GAS: u64 = 10_000_000;

/// Execute a transaction against a persistent world (state carries across calls)
fn execute_on_world(
    world: &mut ScenarioWorld,
    function: &str,
    caller: &str,
    gas_limit: u64,
    target_address: &str,
    args: &[&str],
) -> (TransactionResult, serde_json::Value) {
    let before_state = StateSnapshot::empty();

    let mut tx = ScCallStep::new()
        .from(caller)
        .to(target_address)
        .function(function)
        .gas_limit(gas_limit);

    for arg in args {
        tx = tx.argument(*arg);
    }

    tx = tx.no_expect();

    world.run_sc_call_step(&mut tx);

    let after_state = StateSnapshot::empty();
    let result = TransactionResult::from_response(&tx, &before_state, &after_state, gas_limit);

    let estimator = GasEstimator::new();
    let gas_estimate = estimator.estimate(result.gas_used, function, function.len());
    let output = format_result(&result, target_address, &gas_estimate);
    (result, output)
}

fn run_demo() {
    let demo_start = Instant::now();

    println!("==========================================================");
    println!("  MultiversX Local Transaction Simulator - Scenario Test");
    println!("==========================================================");
    println!();
    println!("  Contract : counter (init=5)");
    println!("  State    : {}", DEMO_STATE);
    println!("  Caller   : {}", DEMO_CALLER);
    println!();
    println!("  Running transactions against a SINGLE persistent world.");
    println!("  State mutations carry across steps.");
    println!();

    // ── Boot a single world ──
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::Experimental);
    world.register_contract(
        format!("file:{}", DEMO_CONTRACT).as_str(),
        counter::ContractBuilder,
    );
    world.register_contract(
        "file:piggybank/output/piggybank.wasm",
        piggybank::ContractBuilder,
    );
    let state_config = StateConfig::from_file(DEMO_STATE).expect("Failed to load demo state");
    state_config
        .apply_to_world(&mut world)
        .expect("Failed to apply state");

    let mut step = 1u8;
    let mut passed = 0u8;
    let mut failed = 0u8;

    // ── Step 1: Read initial counter value → expect 5 ──
    print_step(step, "Read initial counter value (expect 0x05)");
    let (result, output) =
        execute_on_world(&mut world, "get", DEMO_CALLER, DEMO_GAS, DEMO_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![5]],
        "get() returned 0x05",
        &mut passed,
        &mut failed,
    );

    // ── Step 2: Increment counter (5 → 6) ──
    step += 1;
    print_step(step, "Increment counter (5 -> 6)");
    let (result, output) = execute_on_world(
        &mut world,
        "increment",
        DEMO_CALLER,
        DEMO_GAS,
        DEMO_TARGET,
        &[],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "increment() succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 3: Read counter again → expect 6 (proves state persisted) ──
    step += 1;
    print_step(step, "Read counter after increment (expect 0x06)");
    let (result, output) =
        execute_on_world(&mut world, "get", DEMO_CALLER, DEMO_GAS, DEMO_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![6]],
        "get() returned 0x06 (state mutation verified)",
        &mut passed,
        &mut failed,
    );

    // ── Step 4: Increment again (6 → 7) ──
    step += 1;
    print_step(step, "Increment counter again (6 -> 7)");
    let (result, output) = execute_on_world(
        &mut world,
        "increment",
        DEMO_CALLER,
        DEMO_GAS,
        DEMO_TARGET,
        &[],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "increment() succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 5: Read → expect 7 ──
    step += 1;
    print_step(step, "Read counter (expect 0x07 - two increments from 5)");
    let (result, output) =
        execute_on_world(&mut world, "get", DEMO_CALLER, DEMO_GAS, DEMO_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![7]],
        "get() returned 0x07 (two increments verified)",
        &mut passed,
        &mut failed,
    );

    // ── Step 6: Call non-existent function → expect error code 1 ──
    step += 1;
    print_step(step, "Call non-existent function (expect error)");
    let (result, output) = execute_on_world(
        &mut world,
        "does_not_exist",
        DEMO_CALLER,
        DEMO_GAS,
        DEMO_TARGET,
        &[],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| matches!(r.status, response::ExecutionStatus::Failed { code: 1, .. }),
        "VM returned error code 1: invalid function",
        &mut passed,
        &mut failed,
    );

    // ── Step 7: Read after failed tx → expect still 7 (failed tx didn't mutate state) ──
    step += 1;
    print_step(step, "Read counter after failed tx (expect still 0x07)");
    let (result, output) =
        execute_on_world(&mut world, "get", DEMO_CALLER, DEMO_GAS, DEMO_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![7]],
        "State unchanged after failed tx",
        &mut passed,
        &mut failed,
    );

    // ── Step 8: Missing account panic → catch gracefully ──
    step += 1;
    print_step(step, "Call from non-existent account (catch VM panic)");

    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let panic_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        execute_on_world(
            &mut world,
            "increment",
            "address:nonexistent",
            DEMO_GAS,
            DEMO_TARGET,
            &[],
        )
    }));
    panic::set_hook(prev_hook);

    match panic_result {
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };
            println!("  [CAUGHT VM PANIC] {}", msg);
            println!("  PASS: Panic caught gracefully instead of crashing");
            passed += 1;
        }
        Ok(_) => {
            println!("  FAIL: Expected a panic but call succeeded");
            failed += 1;
        }
    }
    println!();

    // ── Summary ──
    let elapsed = demo_start.elapsed();
    println!("==========================================================");
    println!(
        "  {} steps | {} passed | {} failed | {:.2}s",
        step,
        passed,
        failed,
        elapsed.as_secs_f64()
    );
    if failed == 0 {
        println!("  ALL TESTS PASSED");
    } else {
        println!("  SOME TESTS FAILED");
    }
    println!("==========================================================");

    if failed > 0 {
        std::process::exit(1);
    }
}

// ── F5: Piggybank Scenario Test ──────────────────────────────────────────

const PB_CONTRACT: &str = "piggybank/output/piggybank.wasm";
const PB_STATE: &str = "examples/piggybank_initial.json";
const PB_CALLER: &str = "address:owner";
const PB_TARGET: &str = "sc:piggybank";
const PB_GAS: u64 = 10_000_000;

fn run_demo_piggybank() {
    let demo_start = Instant::now();

    println!("==========================================================");
    println!("  MultiversX Local TX Simulator - Piggybank Scenario Test");
    println!("==========================================================");
    println!();
    println!("  Contract : piggybank (target=100, require! validation)");
    println!("  State    : {}", PB_STATE);
    println!("  Caller   : {}", PB_CALLER);
    println!();
    println!("  Patterns : require! macro, 3x storage mappers,");
    println!("             conditional logic, deposit/withdraw flow");
    println!();

    // ── Boot a single world ──
    let mut world = ScenarioWorld::new().executor_config(ExecutorConfig::Experimental);
    world.register_contract(
        format!("file:{}", PB_CONTRACT).as_str(),
        piggybank::ContractBuilder,
    );
    world.register_contract(
        format!("file:{}", DEMO_CONTRACT).as_str(),
        counter::ContractBuilder,
    );
    let state_config = StateConfig::from_file(PB_STATE).expect("Failed to load piggybank state");
    state_config
        .apply_to_world(&mut world)
        .expect("Failed to apply state");

    let mut step = 0u8;
    let mut passed = 0u8;
    let mut failed = 0u8;

    // ── Step 1: Read initial total → expect 0 ──
    step += 1;
    print_step(step, "Read initial total (expect 0x00)");
    let (result, output) =
        execute_on_world(&mut world, "getTotal", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![Vec::<u8>::new()],
        "getTotal() returned 0 (empty piggybank)",
        &mut passed,
        &mut failed,
    );

    // ── Step 2: Read target → expect 100 (0x64) ──
    step += 1;
    print_step(step, "Read savings target (expect 0x64 = 100)");
    let (result, output) =
        execute_on_world(&mut world, "getTarget", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![100]],
        "getTarget() returned 0x64 (target=100)",
        &mut passed,
        &mut failed,
    );

    // ── Step 3: Deposit 30 ──
    step += 1;
    print_step(step, "Deposit 30 into piggybank");
    let (result, output) = execute_on_world(
        &mut world,
        "deposit",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &["0x1e"],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "deposit(30) succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 4: Read total → expect 30 (0x1e) ──
    step += 1;
    print_step(step, "Read total after deposit (expect 0x1e = 30)");
    let (result, output) =
        execute_on_world(&mut world, "getTotal", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![30]],
        "getTotal() returned 0x1e (30)",
        &mut passed,
        &mut failed,
    );

    // ── Step 5: Check status → expect 0 (still collecting) ──
    step += 1;
    print_step(step, "Check status (expect 0x00 = collecting)");
    let (result, output) =
        execute_on_world(&mut world, "getStatus", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![Vec::<u8>::new()],
        "getStatus() returned 0 (still collecting)",
        &mut passed,
        &mut failed,
    );

    // ── Step 6: Deposit 50 ──
    step += 1;
    print_step(step, "Deposit 50 into piggybank");
    let (result, output) = execute_on_world(
        &mut world,
        "deposit",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &["0x32"],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "deposit(50) succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 7: Deposit 30 more ──
    step += 1;
    print_step(step, "Deposit 30 more (total should reach 110)");
    let (result, output) = execute_on_world(
        &mut world,
        "deposit",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &["0x1e"],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "deposit(30) succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 8: Read total → expect 110 (0x6e) ──
    step += 1;
    print_step(step, "Read total (expect 0x6e = 110)");
    let (result, output) =
        execute_on_world(&mut world, "getTotal", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![110]],
        "getTotal() returned 0x6e (110)",
        &mut passed,
        &mut failed,
    );

    // ── Step 9: Check status → expect 1 (target reached!) ──
    step += 1;
    print_step(step, "Check status (expect 0x01 = TARGET REACHED!)");
    let (result, output) =
        execute_on_world(&mut world, "getStatus", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![1]],
        "getStatus() returned 1 (target reached!)",
        &mut passed,
        &mut failed,
    );

    // ── Step 10: Check deposit count → expect 3 ──
    step += 1;
    print_step(step, "Read deposit count (expect 0x03)");
    let (result, output) = execute_on_world(
        &mut world,
        "getNumDeposits",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &[],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![3]],
        "getNumDeposits() returned 3",
        &mut passed,
        &mut failed,
    );

    // ── Step 11: Withdraw 10 ──
    step += 1;
    print_step(step, "Withdraw 10 from piggybank");
    let (result, output) = execute_on_world(
        &mut world,
        "withdraw",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &["0x0a"],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success,
        "withdraw(10) succeeded",
        &mut passed,
        &mut failed,
    );

    // ── Step 12: Read total → expect 100 (0x64) ──
    step += 1;
    print_step(step, "Read total after withdraw (expect 0x64 = 100)");
    let (result, output) =
        execute_on_world(&mut world, "getTotal", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![100]],
        "getTotal() returned 0x64 (100)",
        &mut passed,
        &mut failed,
    );

    // ── Step 13: Withdraw 200 → expect FAIL (require! "Insufficient funds") ──
    step += 1;
    print_step(step, "Withdraw 200 (expect FAIL: insufficient funds)");
    let (result, output) = execute_on_world(
        &mut world,
        "withdraw",
        PB_CALLER,
        PB_GAS,
        PB_TARGET,
        &["0xc8"],
    );
    print_json(&output);
    assert_step(
        &result,
        |r| matches!(r.status, response::ExecutionStatus::Failed { code: 4, .. }),
        "withdraw(200) failed with require! error",
        &mut passed,
        &mut failed,
    );

    // ── Step 14: Read total after failed tx → expect still 100 ──
    step += 1;
    print_step(step, "Read total after failed withdraw (expect still 0x64)");
    let (result, output) =
        execute_on_world(&mut world, "getTotal", PB_CALLER, PB_GAS, PB_TARGET, &[]);
    print_json(&output);
    assert_step(
        &result,
        |r| r.status == response::ExecutionStatus::Success && r.output == vec![vec![100]],
        "State unchanged after failed tx (still 100)",
        &mut passed,
        &mut failed,
    );

    // ── Summary ──
    let elapsed = demo_start.elapsed();
    println!("==========================================================");
    println!(
        "  {} steps | {} passed | {} failed | {:.2}s",
        step,
        passed,
        failed,
        elapsed.as_secs_f64()
    );
    if failed == 0 {
        println!("  ALL TESTS PASSED");
    } else {
        println!("  SOME TESTS FAILED");
    }
    println!("==========================================================");

    if failed > 0 {
        std::process::exit(1);
    }
}

fn print_step(num: u8, description: &str) {
    println!("----------------------------------------------------------");
    println!("  Step {}: {}", num, description);
    println!("----------------------------------------------------------");
}

fn print_json(output: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(output).unwrap());
}

fn assert_step(
    result: &TransactionResult,
    check: impl FnOnce(&TransactionResult) -> bool,
    label: &str,
    passed: &mut u8,
    failed: &mut u8,
) {
    if check(result) {
        println!("  PASS: {}", label);
        *passed += 1;
    } else {
        println!("  FAIL: {}", label);
        println!("  Got: {:?}", result.status);
        if !result.output.is_empty() {
            let hex_vals: Vec<String> = result.output.iter().map(hex::encode).collect();
            println!("  Output: [{}]", hex_vals.join(", "));
        }
        *failed += 1;
    }
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
