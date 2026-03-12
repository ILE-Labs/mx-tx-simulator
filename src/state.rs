use crate::errors::SimulationError;
use multiversx_sc_scenario::{
    scenario_model::{Account, SetStateStep},
    ScenarioWorld,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct StateConfig {
    pub accounts: HashMap<String, AccountConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AccountConfig {
    #[serde(default)]
    pub nonce: u64,
    pub balance: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub storage: HashMap<String, String>,
}

impl StateConfig {
    /// Load state configuration from a JSON file
    pub fn from_file(path: &str) -> Result<Self, SimulationError> {
        // Check if file exists
        if !Path::new(path).exists() {
            return Err(SimulationError::StateFileNotFound(path.to_string()));
        }

        // Read file contents
        let contents = fs::read_to_string(path).map_err(|e| SimulationError::InvalidStateFile {
            path: path.to_string(),
            reason: format!("Failed to read file: {}", e),
        })?;

        // Parse JSON
        let config: StateConfig =
            serde_json::from_str(&contents).map_err(|e| SimulationError::InvalidStateFile {
                path: path.to_string(),
                reason: format!("JSON parse error: {}", e),
            })?;

        // Validate contract code paths exist
        for (addr, account_config) in &config.accounts {
            if let Some(code_path) = &account_config.code {
                // Extract actual file path from "file:path" format
                let file_path = if let Some(stripped) = code_path.strip_prefix("file:") {
                    stripped
                } else {
                    code_path.as_str()
                };

                if !Path::new(file_path).exists() {
                    return Err(SimulationError::ContractNotFound(format!(
                        "Contract code for account '{}' not found at: {}",
                        addr, file_path
                    )));
                }
            }
        }

        Ok(config)
    }

    /// Apply this state configuration to a ScenarioWorld
    pub fn apply_to_world(self, world: &mut ScenarioWorld) -> Result<(), SimulationError> {
        let mut set_state = SetStateStep::new();

        for (address, account_config) in self.accounts {
            // Create account with balance
            let mut account = Account::new().balance(account_config.balance.as_str());

            // Set nonce if non-zero
            if account_config.nonce > 0 {
                account = account.nonce(account_config.nonce);
            }

            // Set contract code if provided
            if let Some(code_path) = account_config.code {
                account = account.code(code_path.as_str());
            }

            // Add all storage entries
            for (key, value) in account_config.storage {
                account
                    .storage
                    .insert(key.as_str().into(), value.as_str().into());
            }

            // Add account to state setup
            set_state = set_state.put_account(address.as_str(), account);
        }

        // Apply the state to the world
        world.set_state_step(set_state);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_state() {
        let json = r#"
        {
            "accounts": {
                "address:wallet1": {
                    "nonce": 0,
                    "balance": "1000000000000000000"
                },
                "sc:contract": {
                    "nonce": 0,
                    "balance": "0",
                    "storage": {
                        "str:counter": "5"
                    }
                }
            }
        }
        "#;

        let config: StateConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.accounts.len(), 2);
        assert!(config.accounts.contains_key("address:wallet1"));
        assert!(config.accounts.contains_key("sc:contract"));

        let contract = config.accounts.get("sc:contract").unwrap();
        assert_eq!(contract.storage.get("str:counter"), Some(&"5".to_string()));
    }
}
