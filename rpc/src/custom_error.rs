use {
    jsonrpc_core::{Error, ErrorCode},
    solana_sdk::clock::Slot,
    thiserror::Error,
};

pub const JSON_RPC_SERVER_ERROR_LONG_TERM_STORAGE_SLOT_SKIPPED: i64 = -32009;
pub const JSON_RPC_SERVER_ERROR_MIN_CONTEXT_SLOT_NOT_REACHED: i64 = -32016;
pub const JSON_RPC_MYSQL_ERROR: i64 = -32017;

#[derive(Error, Debug)]
pub enum RpcCustomError {
    #[error("LongTermStorageSlotSkipped")]
    LongTermStorageSlotSkipped { slot: Slot },
    #[error("MinContextSlotNotReached")]
    MinContextSlotNotReached { context_slot: Slot },
    #[error("MySQLError")]
    MySQLError { message: String }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinContextSlotNotReachedErrorData {
    pub context_slot: Slot,
}

impl From<RpcCustomError> for Error {
    fn from(e: RpcCustomError) -> Self {
        match e {
            RpcCustomError::LongTermStorageSlotSkipped { slot } => Self {
                code: ErrorCode::ServerError(JSON_RPC_SERVER_ERROR_LONG_TERM_STORAGE_SLOT_SKIPPED),
                message: format!("Slot {slot} was skipped, or missing in long-term storage"),
                data: None,
            },
            RpcCustomError::MinContextSlotNotReached { context_slot } => Self {
                code: ErrorCode::ServerError(JSON_RPC_SERVER_ERROR_MIN_CONTEXT_SLOT_NOT_REACHED),
                message: "Minimum context slot has not been reached".to_string(),
                data: Some(serde_json::json!(MinContextSlotNotReachedErrorData {
                    context_slot,
                })),
            },
            RpcCustomError::MySQLError { message } => Self {
                code: ErrorCode::ServerError(JSON_RPC_MYSQL_ERROR),
                message,
                data: None,
            },
        }
    }
}
