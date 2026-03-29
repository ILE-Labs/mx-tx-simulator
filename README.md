# MultiversX Local Transaction Simulator

A CLI tool that simulates MultiversX smart contract transactions entirely offline. No devnet, no testnet, no waiting for blocks -- just instant execution against a local WASM VM with full JSON output.

Built as a proof-of-concept for the [MultiversX Growth Games Grant](https://multiversx.com).

## Features

- **Local Simulation Engine** -- Executes smart contract calls against the Wasmer WASM runtime via `multiversx-sc-scenario`, with state loaded from JSON files
- **Gas Cost Prediction** -- Estimates gas using the official GasSchedule V8 cost tables (base transaction costs, storage operations, WASM execution)
- **Structured JSON Output** -- Returns execution status, return values, logs, error messages, state changes, and gas breakdown
- **Persistent State** -- State mutations carry across sequential transactions within a session
- **Error Handling** -- Gracefully catches VM panics, `require!` failures, and invalid function calls

## Quick Start

```bash
# Build the project
cargo build

# Run the counter demo (8-step scenario)
cargo run -- demo

# Run the piggybank demo (14-step scenario with require! validation)
cargo run -- demo-piggybank

# Simulate a single transaction
cargo run -- simulate \
  -c counter/output/counter.wasm \
  -f get \
  -u "address:wallet1" \
  -s examples/counter_initial.json
```

## Project Structure

```
src/
  main.rs          # CLI, simulation engine, demo scenarios
  gas.rs           # Gas estimator (GasSchedule V8)
  formatter.rs     # JSON output formatting
  response.rs      # TransactionResult, StateSnapshot, StateDiff
  state.rs         # JSON state file loading
  errors.rs        # Error types

counter/           # Simple counter contract (get/increment)
piggybank/         # Complex contract (deposit/withdraw/require!)
examples/          # JSON state files for demos
```

## State Files

State is defined in JSON. Each account has a nonce, balance, and optional contract code + storage:

```json
{
  "accounts": {
    "address:wallet1": {
      "nonce": 0,
      "balance": "1000000000000000000"
    },
    "sc:target_contract": {
      "nonce": 0,
      "balance": "0",
      "code": "file:counter/output/counter.wasm",
      "storage": {
        "str:counter": "5"
      }
    }
  }
}
```

## Demo Scenarios

### Counter (8 steps)

A simple get/increment contract that verifies:
- State reads and writes
- State persistence across transactions
- Error handling (invalid function, missing account)
- Failed transactions don't mutate state

### Piggybank (14 steps)

A savings contract with `require!` validation that verifies:
- Multiple storage mappers (total, deposits, target)
- Deposit/withdraw flow with balance tracking
- `require!` rejection on insufficient funds
- Conditional logic (savings goal status)
- Failed `require!` leaves state unchanged

## Sample Output

```json
{
  "execution": { "status": "success", "summary": "Transaction executed successfully" },
  "gas": {
    "limit": 10000000,
    "estimated_total": 100150,
    "breakdown": [
      { "operation": "base_transaction_cost (min: 50000, data: 3 bytes x 50)", "cost": 50150 },
      { "operation": "storage_load (50000)", "cost": 50000 }
    ],
    "confidence": "low (schedule estimation only, RustVM returned 0)",
    "method": "GasSchedule V8 costs + WASM opcode metering"
  },
  "return_values": ["0x05"],
  "state_changes": [],
  "logs": [],
  "errors": null
}
```

## CLI Reference

```
Usage: mx-local-simulator <COMMAND>

Commands:
  simulate        Run a single transaction simulation
  demo            Run the counter scenario test
  demo-piggybank  Run the piggybank scenario test

Simulate options:
  -c, --contract <CONTRACT>          Path to compiled .wasm contract
  -f, --function <FUNCTION>          Endpoint to call
  -u, --caller <CALLER>              Caller address (e.g. "address:wallet1")
  -g, --gas-limit <GAS_LIMIT>        Gas limit [default: 10000000]
  -s, --state-file <STATE_FILE>      JSON state file [default: state.json]
  -a, --args <ARGS>                  Comma-separated hex arguments
      --target-address <ADDRESS>     Target SC address [default: sc:target_contract]
```

## Requirements

- Rust 1.78+
- `multiversx-sc-scenario` 0.65.0 with `wasmer-experimental` feature
