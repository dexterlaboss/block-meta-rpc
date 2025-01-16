use {
    crate::{
        request_processor::JsonRpcRequestProcessor,
    },
    jsonrpc_core::{
        BoxFuture,
        Result,
    },
    jsonrpc_derive::rpc,
    solana_rpc_client_api::{
        config::*,
        response::*,
    },
    solana_sdk::{
        clock::{
            Slot,
            UnixTimestamp,
        },
        commitment_config::{
            CommitmentConfig,
        },
    },
};

// Minimal RPC interface
pub mod storage_rpc_minimal {
    use super::*;
    #[rpc]
    pub trait Minimal {
        type Metadata;

        #[rpc(meta, name = "getHealth")]
        fn get_health(&self, meta: Self::Metadata) -> Result<String>;

        #[rpc(meta, name = "getSlot")]
        fn get_slot(&self, meta: Self::Metadata, config: Option<RpcContextConfig>) -> BoxFuture<Result<Slot>>;

        #[rpc(meta, name = "getBlockHeight")]
        fn get_block_height(
            &self,
            meta: Self::Metadata,
            config: Option<RpcContextConfig>,
        ) -> BoxFuture<Result<u64>>;

        #[rpc(meta, name = "getVersion")]
        fn get_version(&self, meta: Self::Metadata) -> Result<RpcVersionInfo>;
    }

    pub struct MinimalImpl;
    impl Minimal for MinimalImpl {
        type Metadata = JsonRpcRequestProcessor;

        fn get_health(&self, _meta: Self::Metadata) -> Result<String> {
            Ok("ok".to_string())
        }

        fn get_slot(&self, meta: Self::Metadata, config: Option<RpcContextConfig>) -> BoxFuture<Result<Slot>> {
            debug!("get_slot rpc request received");
            Box::pin( async move { meta.get_slot(config.unwrap_or_default()).await } )
        }

        fn get_block_height(
            &self,
            meta: Self::Metadata,
            config: Option<RpcContextConfig>,
        ) -> BoxFuture<Result<u64>> {
            debug!("get_block_height rpc request received");
            Box::pin( async move { meta.get_block_height(config.unwrap_or_default()).await } )
        }

        fn get_version(&self, _: Self::Metadata) -> Result<RpcVersionInfo> {
            debug!("get_version rpc request received");
            let version = solana_version::Version::default();
            Ok(RpcVersionInfo {
                solana_core: version.to_string(),
                feature_set: Some(version.feature_set),
            })
        }
    }
}

// Full RPC interface that an API node is expected to provide
// (rpc_minimal should also be provided by an API node)
pub mod storage_rpc_full {
    use {
        super::*,
    };
    #[rpc]
    pub trait Full {
        type Metadata;

        #[rpc(meta, name = "getBlockTime")]
        fn get_block_time(
            &self,
            meta: Self::Metadata,
            slot: Slot,
        ) -> BoxFuture<Result<Option<UnixTimestamp>>>;

        #[rpc(meta, name = "getBlocks")]
        fn get_blocks(
            &self,
            meta: Self::Metadata,
            start_slot: Slot,
            wrapper: Option<RpcBlocksConfigWrapper>,
            config: Option<RpcContextConfig>,
        ) -> BoxFuture<Result<Vec<Slot>>>;

        #[rpc(meta, name = "getBlocksWithLimit")]
        fn get_blocks_with_limit(
            &self,
            meta: Self::Metadata,
            start_slot: Slot,
            limit: usize,
            commitment: Option<CommitmentConfig>,
        ) -> BoxFuture<Result<Vec<Slot>>>;

        #[rpc(meta, name = "getFirstAvailableBlock")]
        fn get_first_available_block(&self, meta: Self::Metadata) -> BoxFuture<Result<Slot>>;
    }

    pub struct FullImpl;
    impl Full for FullImpl {
        type Metadata = JsonRpcRequestProcessor;

        fn get_blocks(
            &self,
            meta: Self::Metadata,
            start_slot: Slot,
            wrapper: Option<RpcBlocksConfigWrapper>,
            config: Option<RpcContextConfig>,
        ) -> BoxFuture<Result<Vec<Slot>>> {
            let (end_slot, maybe_config) =
                wrapper.map(|wrapper| wrapper.unzip()).unwrap_or_default();
            debug!(
                "get_blocks rpc request received: {}-{:?}",
                start_slot, end_slot
            );
            Box::pin(async move {
                meta.get_blocks(start_slot, end_slot, config.or(maybe_config))
                    .await
            })
        }

        fn get_blocks_with_limit(
            &self,
            meta: Self::Metadata,
            start_slot: Slot,
            limit: usize,
            commitment: Option<CommitmentConfig>,
        ) -> BoxFuture<Result<Vec<Slot>>> {
            debug!(
                "get_blocks_with_limit rpc request received: {}-{}",
                start_slot, limit,
            );
            Box::pin(async move {
                meta.get_blocks_with_limit(start_slot, limit, commitment)
                    .await
            })
        }

        fn get_block_time(
            &self,
            meta: Self::Metadata,
            slot: Slot,
        ) -> BoxFuture<Result<Option<UnixTimestamp>>> {
            Box::pin(async move { meta.get_block_time(slot).await })
        }

        fn get_first_available_block(&self, meta: Self::Metadata) -> BoxFuture<Result<Slot>> {
            debug!("get_first_available_block rpc request received");
            Box::pin(async move { Ok(meta.get_first_available_block().await) })
        }
    }
}