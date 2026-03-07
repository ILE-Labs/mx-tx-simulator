use multiversx_sc_scenario::scenario_model::ScCallStep;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub storage: HashMap<String, String>,
}

impl StateSnapshot {
    /// Create an empty snapshot (used when we can't access state directly)
    pub fn empty() -> Self {
        StateSnapshot {
            storage: HashMap::new(),
        }
    }

    /// Manually add a storage entry to the snapshot
    pub fn add_storage(&mut self, key: String, value: String) {
        self.storage.insert(key, value);
    }
}

#[derive(Debug)]
pub struct StateDiff {
    pub storage_changes: HashMap<String, StorageChange>,
}

#[derive(Debug, Clone)]
pub enum StorageChange {
    Added(String),
    Modified { before: String, after: String },
    Deleted(String),
}

impl StateDiff {
    /// Compute diff between two state snapshots
    pub fn compute(before: &StateSnapshot, after: &StateSnapshot) -> Self {
        let mut storage_changes = HashMap::new();

        // Find modified and deleted keys
        for (key, before_value) in &before.storage {
            match after.storage.get(key) {
                Some(after_value) if after_value != before_value => {
                    storage_changes.insert(
                        key.clone(),
                        StorageChange::Modified {
                            before: before_value.clone(),
                            after: after_value.clone(),
                        },
                    );
                }
                None => {
                    storage_changes
                        .insert(key.clone(), StorageChange::Deleted(before_value.clone()));
                }
                _ => {
                    // No change
                }
            }
        }

        // Find added keys
        for (key, after_value) in &after.storage {
            if !before.storage.contains_key(key) {
                storage_changes.insert(key.clone(), StorageChange::Added(after_value.clone()));
            }
        }

        StateDiff { storage_changes }
    }
}

#[derive(Debug)]
pub struct TransactionResult {
    pub status: ExecutionStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Vec<Vec<u8>>,
    pub logs: Vec<String>,
    pub state_diff: StateDiff,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Success,
    Failed { code: i32, reason: String },
}

impl TransactionResult {
    /// Extract transaction result from ScCallStep response
    pub fn from_response(
        tx: &ScCallStep,
        before_state: &StateSnapshot,
        after_state: &StateSnapshot,
        gas_limit: u64,
    ) -> Self {
        let response = tx.response.as_ref().expect("No response from transaction");

        // Determine execution status
        // ReturnCode is an enum, convert to i32
        let status_code = response.tx_error.status as i32;
        let (status, error_message) = if status_code == 0 {
            (ExecutionStatus::Success, None)
        } else {
            (
                ExecutionStatus::Failed {
                    code: status_code,
                    reason: response.tx_error.message.clone(),
                },
                Some(response.tx_error.message.clone()),
            )
        };

        // Compute state diff
        let state_diff = StateDiff::compute(before_state, after_state);

        // Convert logs to strings
        // Note: log.data is Vec<u8>, not Vec<Vec<u8>>
        let logs: Vec<String> = response
            .logs
            .iter()
            .map(|log| {
                // Flatten data if it's nested (based on error message)
                let data_bytes: Vec<u8> = log.data.iter().flat_map(|v| v.iter().copied()).collect();
                let data_str = String::from_utf8_lossy(&data_bytes);
                format!("{}: {}", log.endpoint, data_str)
            })
            .collect();

        TransactionResult {
            status,
            gas_used: response.gas_used,
            gas_limit,
            output: response.out.clone(),
            logs,
            state_diff,
            error_message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_diff_no_changes() {
        let mut before = StateSnapshot::empty();
        before.add_storage("str:counter".to_string(), "5".to_string());

        let mut after = StateSnapshot::empty();
        after.add_storage("str:counter".to_string(), "5".to_string());

        let diff = StateDiff::compute(&before, &after);
        assert_eq!(diff.storage_changes.len(), 0);
    }

    #[test]
    fn test_state_diff_modified() {
        let mut before = StateSnapshot::empty();
        before.add_storage("str:counter".to_string(), "5".to_string());

        let mut after = StateSnapshot::empty();
        after.add_storage("str:counter".to_string(), "6".to_string());

        let diff = StateDiff::compute(&before, &after);
        assert_eq!(diff.storage_changes.len(), 1);

        match diff.storage_changes.get("str:counter") {
            Some(StorageChange::Modified { before: b, after: a }) => {
                assert_eq!(b, "5");
                assert_eq!(a, "6");
            }
            _ => panic!("Expected Modified change"),
        }
    }

    #[test]
    fn test_state_diff_added() {
        let before = StateSnapshot::empty();

        let mut after = StateSnapshot::empty();
        after.add_storage("str:new_key".to_string(), "42".to_string());

        let diff = StateDiff::compute(&before, &after);
        assert_eq!(diff.storage_changes.len(), 1);

        match diff.storage_changes.get("str:new_key") {
            Some(StorageChange::Added(value)) => {
                assert_eq!(value, "42");
            }
            _ => panic!("Expected Added change"),
        }
    }
}
