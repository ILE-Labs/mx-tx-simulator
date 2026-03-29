use crate::gas::GasEstimate;
use crate::response::{ExecutionStatus, StorageChange, TransactionResult};
use serde_json::{json, Value};

/// Format a TransactionResult into a pretty JSON output
pub fn format_result(
    result: &TransactionResult,
    target_address: &str,
    gas_estimate: &GasEstimate,
) -> Value {
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

    // Format gas information with cost breakdown
    let breakdown: Vec<Value> = gas_estimate
        .breakdown
        .iter()
        .map(|item| {
            json!({
                "operation": item.operation,
                "cost": item.cost
            })
        })
        .collect();

    let gas_info = json!({
        "limit": result.gas_limit,
        "estimated_total": gas_estimate.total_estimated,
        "wasm_execution": gas_estimate.wasm_execution,
        "base_cost": gas_estimate.base_cost,
        "storage_ops": gas_estimate.storage_ops,
        "confidence": gas_estimate.confidence,
        "method": gas_estimate.method,
        "breakdown": breakdown
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
    use crate::gas::{GasCostItem, GasEstimate};
    use crate::response::{ExecutionStatus, StateDiff, StorageChange, TransactionResult};
    use std::collections::HashMap;

    fn make_test_gas_estimate() -> GasEstimate {
        GasEstimate {
            wasm_execution: 1000,
            base_cost: 50000,
            storage_ops: 125000,
            total_estimated: 176000,
            confidence: "medium (WASM metering + schedule estimation)".into(),
            method: "GasSchedule V8 costs + WASM opcode metering".into(),
            breakdown: vec![
                GasCostItem {
                    operation: "base_transaction_cost".into(),
                    cost: 50000,
                },
                GasCostItem {
                    operation: "storage_load".into(),
                    cost: 50000,
                },
                GasCostItem {
                    operation: "storage_store".into(),
                    cost: 75000,
                },
                GasCostItem {
                    operation: "wasm_execution (measured)".into(),
                    cost: 1000,
                },
            ],
        }
    }

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

        let gas_estimate = make_test_gas_estimate();
        let output = format_result(&result, "sc:target_contract", &gas_estimate);

        assert_eq!(output["execution"]["status"], "success");
        assert_eq!(output["gas"]["estimated_total"], 176000);
        assert_eq!(output["gas"]["wasm_execution"], 1000);
        assert_eq!(output["gas"]["base_cost"], 50000);
        assert_eq!(output["gas"]["storage_ops"], 125000);
        assert_eq!(output["return_values"][0], "0x06");
        assert!(output["errors"].is_null());

        // Check gas breakdown
        let breakdown = output["gas"]["breakdown"].as_array().unwrap();
        assert_eq!(breakdown.len(), 4);
        assert_eq!(breakdown[0]["operation"], "base_transaction_cost");

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

        let gas_estimate = make_test_gas_estimate();
        let output = format_result(&result, "sc:target_contract", &gas_estimate);

        assert_eq!(output["execution"]["status"], "failed");
        assert_eq!(output["errors"]["code"], 4);
        assert_eq!(output["errors"]["message"], "function not found");
    }
}
