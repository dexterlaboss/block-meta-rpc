use {
    crate::{
        custom_error::RpcCustomError,
    },
    jsonrpc_core::{
        Error, Metadata, Result
    },
    solana_rpc_client_api::{
        config::*,
        request::{
            MAX_GET_CONFIRMED_BLOCKS_RANGE,
        },
    },
    solana_sdk::{
        clock::{
            Slot,
            UnixTimestamp,
        },
        commitment_config::{
            CommitmentConfig,
        },
        exit::Exit,
    },
    solana_storage_mysql::{
        meta_storage,
        mysql::{
            MySQLConfig,
        }
    },
    std::{
        sync::{
            Arc,
            RwLock,
        },
    },
};

pub const MAX_REQUEST_BODY_SIZE: usize = 50 * (1 << 10); // 50kB

pub(crate) fn check_is_at_least_confirmed(commitment: CommitmentConfig) -> Result<()> {
    if !commitment.is_at_least_confirmed() {
        return Err(Error::invalid_params(
            "Method does not support commitment below `confirmed`",
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockCheck {
    pub exists: bool,
}

#[derive(Debug, Default, Clone)]
pub struct JsonRpcConfig {
    pub rpc_mysql_config: Option<MySQLConfig>,
    pub rpc_threads: usize,
    pub rpc_niceness_adj: i8,
    pub full_api: bool,
    pub obsolete_v1_7_api: bool,
    pub max_request_body_size: Option<usize>,
}

impl JsonRpcConfig {
    pub fn default_for_storage_rpc() -> Self {
        Self {
            full_api: true,
            ..Self::default()
        }
    }
}



pub struct JsonRpcRequestProcessor {
    config: JsonRpcConfig,
    #[allow(dead_code)]
    rpc_service_exit: Arc<RwLock<Exit>>,
    metadata_storage: Option<meta_storage::MetaStorage>,
}

impl Metadata for JsonRpcRequestProcessor {}

impl Clone for JsonRpcRequestProcessor {
    fn clone(&self) -> Self {
        JsonRpcRequestProcessor {
            config: self.config.clone(),
            rpc_service_exit: Arc::clone(&self.rpc_service_exit),
            metadata_storage: self.metadata_storage.clone(),
        }
    }
}

impl JsonRpcRequestProcessor {
    fn genesis_creation_time(&self) -> UnixTimestamp {
        // TODO: Get genesis creation time from config?
        0
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: JsonRpcConfig,
        rpc_service_exit: Arc<RwLock<Exit>>,
        metadata_storage: Option<meta_storage::MetaStorage>,
    ) -> Self {
        Self {
            config,
            rpc_service_exit,
            metadata_storage,
        }
    }

    fn check_storage_result<T>(
        &self,
        result: &std::result::Result<T, meta_storage::Error>,
    ) -> Result<()> {
        info!("Checking mysql block");
        if let Err(e) = result {
            info!("Block error: {}", e);
        }
        if let Err(meta_storage::Error::BlockNotFound(slot)) = result {
            return Err(RpcCustomError::LongTermStorageSlotSkipped { slot: *slot }.into());
        }
        info!("Block check successful");
        Ok(())
    }

    pub async fn get_blocks(
        &self,
        start_slot: Slot,
        // FIXME: Maybe make this non-optional?
        end_slot: Option<Slot>,
        config: Option<RpcContextConfig>,
    ) -> Result<Vec<Slot>> {
        let config = config.unwrap_or_default();
        let commitment = config.commitment.unwrap_or_default();
        check_is_at_least_confirmed(commitment)?;

        if end_slot.unwrap() < start_slot {
            return Ok(vec![]);
        }
        if end_slot.unwrap() - start_slot > MAX_GET_CONFIRMED_BLOCKS_RANGE {
            return Err(Error::invalid_params(format!(
                "Slot range too large; max {MAX_GET_CONFIRMED_BLOCKS_RANGE}"
            )));
        }

        if let Some(metadata_storage) = &self.metadata_storage {
            return metadata_storage
                .get_confirmed_blocks(start_slot, (end_slot.unwrap() - start_slot) as usize + 1) // increment limit by 1 to ensure returned range is inclusive of both start_slot and end_slot
                .await
                .map(|mut mysql_blocks| {
                    mysql_blocks.retain(|&slot| slot <= end_slot.unwrap());
                    mysql_blocks
                })
                .map_err(|_| {
                    Error::invalid_params(
                        "MySQL query failed (maybe timeout due to too large range?)"
                            .to_string(),
                    )
                });
        }

        Ok(vec![])
    }

    pub async fn get_blocks_with_limit(
        &self,
        start_slot: Slot,
        limit: usize,
        commitment: Option<CommitmentConfig>,
    ) -> Result<Vec<Slot>> {
        let commitment = commitment.unwrap_or_default();
        check_is_at_least_confirmed(commitment)?;

        if limit > MAX_GET_CONFIRMED_BLOCKS_RANGE as usize {
            return Err(Error::invalid_params(format!(
                "Limit too large; max {MAX_GET_CONFIRMED_BLOCKS_RANGE}"
            )));
        }

        if let Some(metadata_storage) = &self.metadata_storage {
            return Ok(metadata_storage
                .get_confirmed_blocks(start_slot, limit)
                .await
                .unwrap_or_default());
        }

        Ok(vec![])
    }

    pub async fn get_block_time(&self, slot: Slot) -> Result<Option<UnixTimestamp>> {
        // Handle the special case for slot 0
        if slot == 0 {
            return Ok(Some(self.genesis_creation_time()));
        }

        // Check if MySQL metadata storage is available
        if let Some(metadata_storage) = &self.metadata_storage {

            let storage_result = metadata_storage.get_block_time(slot).await;
            self.check_storage_result(&storage_result)?;
            return Ok(storage_result
                .ok()
                .and_then(|naive_datetime| Some(naive_datetime.timestamp())));
        }

        // Return None if MySQL metadata storage is not available
        Ok(None)
    }

    pub async fn get_block_height(&self, _config: RpcContextConfig) -> Result<u64> {
        // Check if MySQL metadata storage is available
        if let Some(metadata_storage) = &self.metadata_storage {

            let storage_result = metadata_storage.get_block_height().await;
            self.check_storage_result(&storage_result)?;
            if let Ok(block_height) = storage_result {
                return Ok(block_height);
            }
        }

        // Return None if MySQL metadata storage is not available
        Ok(0)
    }

    pub async fn get_first_available_block(&self) -> Slot {
        if let Some(metadata_storage) = &self.metadata_storage {
            let first_slot = metadata_storage
                .get_first_available_block()
                .await
                .unwrap_or(None)
                .unwrap_or(Slot::default());

            return first_slot;
        }
        Slot::default()
    }

    pub async fn get_slot(&self, _config: RpcContextConfig) -> Result<Slot> {
        if let Some(metadata_storage) = &self.metadata_storage {
            let first_slot = metadata_storage
                .get_slot()
                .await
                .unwrap_or(None)
                .unwrap_or(Slot::default());

            return Ok(first_slot);
        }
        Ok(Slot::default())
    }
}



