use crate::response::{ExecutionStatus, StorageChange, TransactionResult};
use serde_json::{json, Value};

/// Format a TransactionResult into a pretty JSON output
pub fn format_result(result: &TransactionResult, target_address: &str) -> Value {
    // Format execution status
    let (status_str, summary) = match &result.status {
        ExecutionStatus::Success => ("success", "Transaction executed successfully"),
        ExecutionStatus::Failed { code, .. } => (
            "failed",
            match *code {
                4 => "Transaction failed: function not found or invalid signature",
                10 => "Transaction failed: insufficient funds",
                _ => "Transaction execution failed",
            },
        ),
    };

    // Format gas information
    let gas_info = json!({
        "limit": result.gas_limit,
        "used": result.gas_used,
        "remaining": result.gas_limit.saturating_sub(result.gas_used),
        "confidence": "high (VM Dry-Run)"
    });

    // Format state changes
    let state_changes = if result.state_diff.storage_changes.is_empty() {
        json!([])
    } else {
        let storage_changes: Vec<Value> = result
            .state_diff
            .storage_changes
            .iter()
            .map(|(key, change)| match change {
                StorageChange::Modified { before, after } => json!({
                    "key": key,
                    "before": before,
                    "after": after,
                    "type": "modified"
                }),
                StorageChange::Added(value) => json!({
                    "key": key,
                    "before": null,
                    "after": value,
                    "type": "added"
                }),
                StorageChange::Deleted(value) => json!({
                    "key": key,
                    "before": value,
                    "after": null,
                    "type": "deleted"
                }),
            })
            .collect();

        json!([{
            "account": target_address,
            "storage": storage_changes
        }])
    };

    // Format return values as hex strings
    let return_values: Vec<String> = result
        .output
        .iter()
        .map(|bytes| {
            if bytes.is_empty() {
                "0x".to_string()
            } else {
                format!("0x{}", hex::encode(bytes))
            }
        })
        .collect();

    // Format error information
    let errors = result.error_message.as_ref().map(|msg| {
        json!({
            "code": match &result.status {
                ExecutionStatus::Failed { code, .. } => *code,
                _ => 0,
            },
            "message": msg
        })
    });

    // Build the complete output
    json!({
        "execution": {
            "status": status_str,
            "summary": summary
        },
        "gas": gas_info,
        "state_changes": state_changes,
        "return_values": return_values,
        "logs": result.logs,
        "errors": errors
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::{ExecutionStatus, StateDiff, StorageChange, TransactionResult};
    use std::collections::HashMap;

    #[test]
    fn test_format_success_result() {
        let mut storage_changes = HashMap::new();
        storage_changes.insert(
            "str:counter".to_string(),
            StorageChange::Modified {
                before: "5".to_string(),
                after: "6".to_string(),
            },
        );

        let result = TransactionResult {
            status: ExecutionStatus::Success,
            gas_used: 2500000,
            gas_limit: 10000000,
            output: vec![vec![6]], // Return value: 6
            logs: vec![],
            state_diff: StateDiff { storage_changes },
            error_message: None,
        };

        let output = format_result(&result, "sc:target_contract");

        assert_eq!(output["execution"]["status"], "success");
        assert_eq!(output["gas"]["used"], 2500000);
        assert_eq!(output["gas"]["remaining"], 7500000);
        assert_eq!(output["return_values"][0], "0x06");
        assert!(output["errors"].is_null());

        // Check state changes
        let changes = &output["state_changes"][0]["storage"];
        assert_eq!(changes[0]["key"], "str:counter");
        assert_eq!(changes[0]["before"], "5");
        assert_eq!(changes[0]["after"], "6");
        assert_eq!(changes[0]["type"], "modified");
    }

    #[test]
    fn test_format_failed_result() {
        let result = TransactionResult {
            status: ExecutionStatus::Failed {
                code: 4,
                reason: "function not found".to_string(),
            },
            gas_used: 1500000,
            gas_limit: 10000000,
            output: vec![],
            logs: vec![],
            state_diff: StateDiff {
                storage_changes: HashMap::new(),
            },
            error_message: Some("function not found".to_string()),
        };

        let output = format_result(&result, "sc:target_contract");

        assert_eq!(output["execution"]["status"], "failed");
        assert_eq!(output["errors"]["code"], 4);
        assert_eq!(output["errors"]["message"], "function not found");
    }
}
