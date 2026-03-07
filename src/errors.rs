use std::fmt;

#[derive(Debug)]
pub enum SimulationError {
    StateFileNotFound(String),
    InvalidStateFile { path: String, reason: String },
    ContractNotFound(String),
    InvalidArgument { index: usize, reason: String },
    ExecutionFailed { code: i32, message: String },
    InvalidHexArgument(String),
    FileIOError(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimulationError::StateFileNotFound(path) => {
                write!(
                    f,
                    "State file not found: {}\nMake sure the file path is correct and the file exists.",
                    path
                )
            }
            SimulationError::InvalidStateFile { path, reason } => {
                write!(
                    f,
                    "Invalid state file '{}':\n{}\nCheck JSON format and ensure all file paths are correct.",
                    path, reason
                )
            }
            SimulationError::ContractNotFound(path) => {
                write!(
                    f,
                    "Contract WASM not found: {}\nEnsure the contract is compiled and the path is correct.",
                    path
                )
            }
            SimulationError::InvalidArgument { index, reason } => {
                write!(
                    f,
                    "Argument at index {} is invalid: {}\nArguments must be hex-encoded strings (e.g., '0x1234' or '1234').",
                    index, reason
                )
            }
            SimulationError::ExecutionFailed { code, message } => {
                write!(
                    f,
                    "Transaction execution failed with code {}:\n{}",
                    code, message
                )
            }
            SimulationError::InvalidHexArgument(arg) => {
                write!(
                    f,
                    "Invalid hex argument: '{}'\nHex strings should contain only valid hex characters (0-9, a-f, A-F) and optionally start with '0x'.",
                    arg
                )
            }
            SimulationError::FileIOError(msg) => {
                write!(f, "File I/O error: {}", msg)
            }
        }
    }
}

impl std::error::Error for SimulationError {}

impl From<std::io::Error> for SimulationError {
    fn from(err: std::io::Error) -> Self {
        SimulationError::FileIOError(err.to_string())
    }
}

impl From<serde_json::Error> for SimulationError {
    fn from(err: serde_json::Error) -> Self {
        SimulationError::InvalidStateFile {
            path: String::from("unknown"),
            reason: err.to_string(),
        }
    }
}
