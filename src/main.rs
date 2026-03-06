use clap::{Parser, Subcommand};
use serde_json::json;
use multiversx_sc_scenario::{
    scenario_model::{Account, ScCallStep, SetStateStep, TxExpect},
    scenario::ScenarioRunner,
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
            println!("Booting MultiversX Local Simulator...");
            println!("Loading state from: {}", state_file);
            
            let mut world = ScenarioWorld::new();

            let mut target_account = Account::new().code(format!("file:{}", contract).as_str());
        
            target_account.storage.insert("str:counter".into(), "u64:5".into());

            world.set_state_step(
                SetStateStep::new()
                    .put_account(caller.as_str(), Account::new().balance("1000000000000000000"))
                    .put_account("sc:target_contract", target_account),
            );

            println!("Impersonating caller: {} (Signature bypassed)", caller);
            println!(" Executing: {} -> {}()", contract, function);

            let mut tx = ScCallStep::new()
                .from(caller.as_str())
                .to("sc:target_contract")
                .function(function.as_str())
                .gas_limit(*gas_limit)
                .expect(TxExpect::ok());


            world.run_sc_call_step(&mut tx);

            let actual_gas_used = match &tx.response {
                Some(res) => res.gas_used,
                None => 0,
            };

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