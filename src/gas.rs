use multiversx_chain_vm::schedule::{GasSchedule, GasScheduleVersion};

/// Gas estimator inspired by the GoVM's two-level gas model:
/// Level 1: WASM opcode costs (from Experimental executor)
/// Level 2: API call costs (from GasSchedule)
pub struct GasEstimator {
    schedule: GasSchedule,
}

/// Final gas estimation result with breakdown
pub struct GasEstimate {
    pub wasm_execution: u64,
    pub base_cost: u64,
    pub storage_ops: u64,
    pub total_estimated: u64,
    pub confidence: String,
    pub method: String,
    pub breakdown: Vec<GasCostItem>,
}

/// Individual gas cost item for the breakdown
pub struct GasCostItem {
    pub operation: String,
    pub cost: u64,
}

impl GasEstimator {
    /// Create a new gas estimator with the latest gas schedule (V8)
    pub fn new() -> Self {
        GasEstimator {
            schedule: GasScheduleVersion::V8.load_gas_schedule(),
        }
    }

    /// Base transaction cost: minimum gas + data byte costs
    /// On MultiversX: min_gas_limit = 50,000, data = 1,500 gas/byte
    fn estimate_base_cost(&self, data_len: usize) -> (u64, GasCostItem) {
        let min_gas: u64 = 50_000;
        let data_gas = data_len as u64 * self.schedule.base_operation_cost.data_copy_per_byte;
        let total = min_gas + data_gas;

        (
            total,
            GasCostItem {
                operation: format!(
                    "base_transaction_cost (min: {}, data: {} bytes x {})",
                    min_gas, data_len, self.schedule.base_operation_cost.data_copy_per_byte
                ),
                cost: total,
            },
        )
    }

    /// Estimate storage operation costs based on the function being called.
    /// Every SC call does at least 1 storage_load. Write operations add storage_store.
    fn estimate_storage_costs(&self, _function: &str, is_view: bool) -> (u64, Vec<GasCostItem>) {
        let mut items = Vec::new();
        let mut total = 0u64;

        // Every SC call reads from storage (at minimum the contract code)
        let load_cost = self.schedule.base_ops_api_cost.storage_load;
        items.push(GasCostItem {
            operation: format!("storage_load ({})", load_cost),
            cost: load_cost,
        });
        total += load_cost;

        // Non-view functions write to storage
        if !is_view {
            let store_cost = self.schedule.base_ops_api_cost.storage_store;
            items.push(GasCostItem {
                operation: format!("storage_store ({})", store_cost),
                cost: store_cost,
            });
            total += store_cost;
        }

        (total, items)
    }

    /// Combine all gas sources into a final prediction
    pub fn estimate(&self, wasm_gas_used: u64, function: &str, data_len: usize) -> GasEstimate {
        // Determine if this is likely a view function
        let is_view = function == "get"
            || function.starts_with("get_")
            || function.starts_with("view_")
            || function == "balance"
            || function == "totalSupply"
            || function == "name"
            || function == "symbol";

        let (base_cost, base_item) = self.estimate_base_cost(data_len);
        let (storage_ops, storage_items) = self.estimate_storage_costs(function, is_view);

        // Build breakdown
        let mut breakdown = vec![base_item];
        breakdown.extend(storage_items);

        if wasm_gas_used > 0 {
            breakdown.push(GasCostItem {
                operation: "wasm_execution (measured)".into(),
                cost: wasm_gas_used,
            });
        }

        let total = base_cost + storage_ops + wasm_gas_used;

        let confidence = if wasm_gas_used > 0 {
            "medium (WASM metering + schedule estimation)".into()
        } else {
            "low (schedule estimation only, RustVM returned 0)".into()
        };

        GasEstimate {
            wasm_execution: wasm_gas_used,
            base_cost,
            storage_ops,
            total_estimated: total,
            confidence,
            method: "GoVM-inspired: GasSchedule V8 costs + WASM opcode metering".into(),
            breakdown,
        }
    }
}
