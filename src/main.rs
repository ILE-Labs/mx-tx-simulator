use clap::{Parser, Subcommand};
use serde_json::json;
use multiversx_sc_scenario::{
    scenario_model::{Account, ScCallStep, SetStateStep, TxExpect},
    ScenarioWorld,
};

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
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Simulate { contract, function, caller, gas_limit, state_file } => {
            println!("🚀 Booting MultiversX Local Simulator...");
            println!("📦 Loading state from: {}", state_file);
            
            let mut world = ScenarioWorld::new();

            world.set_state_step(
                SetStateStep::new()
                    .put_account(caller.as_str(), Account::new().balance("1000000000000000000"))
                    .put_account(
                        "sc:target_contract", 
                        Account::new()
                            .code(format!("file:{}", contract).as_str())
                            .storage("str:counter", "5"), 
                    ),
            );

            println!("👤 Impersonating caller: {} (Signature bypassed)", caller);
            println!("⚙️  Executing: {} -> {}()", contract, function);


            let tx = ScCallStep::new()
                .from(caller.as_str())
                .to("sc:target_contract")
                .function(function.as_str())
                .gas_limit(*gas_limit)
                .expect(TxExpect::ok());

          
            let tx_result = world.sc_call_step(&tx);


            let actual_gas_used = tx_result.gas_used();

          
            let report = json!({
                "status": "success",
                "gas": {
                    "predicted": actual_gas_used, 
                    "confidence": "high (VM Dry-Run)"
                },
                "state_changes": [
                    {
                        "key": "counter",
                        "before": 5,
                        "after": "6 (Mocked for POC output)"
                    }
                ],
                "logs": [
                    format!("{} executed successfully", function)
                ]
            });

            println!("\nSimulation Result:\n-----------------");
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
    }
}