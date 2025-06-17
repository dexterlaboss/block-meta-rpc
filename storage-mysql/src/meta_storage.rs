use {
    crate::{
        mysql::{
            DEFAULT_PORT,
            DEFAULT_HOST,
            MySQLConnection,
        },
    },
    log::*,
    solana_sdk::{
        clock::{
            Slot,
        },
    },
    std::{
        boxed::Box,
        str::FromStr,
    },
    thiserror::Error,
    tokio::task::JoinError,
    time::PrimitiveDateTime,
    chrono::{DateTime, Utc},
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Storage Error: {0}")]
    StorageBackendError(Box<dyn std::error::Error + Send>),

    #[error("I/O Error: {0}")]
    IoError(std::io::Error),

    #[error("Transaction encoded is not supported")]
    UnsupportedTransactionEncoding,

    #[error("Block not found: {0}")]
    BlockNotFound(Slot),

    #[error("Signature not found")]
    SignatureNotFound,

    #[error("tokio error")]
    TokioJoinError(JoinError),
}

impl From<crate::mysql::Error> for Error {
    fn from(err: crate::mysql::Error) -> Self {
        Self::StorageBackendError(Box::new(err))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn slot_to_key(slot: Slot) -> String {
    slot.to_string()
}

pub fn key_to_slot(key: &str) -> Option<Slot> {
    match Slot::from_str(key) {
        Ok(slot) => Some(slot),
        Err(err) => {
            // bucket data is probably corrupt
            warn!("Failed to parse object key as a slot: {}: {}", key, err);
            None
        }
    }
}

#[derive(Debug)]
pub struct MetaStorageConfig {
    pub read_only: bool,
    pub timeout: Option<std::time::Duration>,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: String,
}

impl Default for MetaStorageConfig {
    fn default() -> Self {
        Self {
            read_only: true,
            timeout: None,
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            username: String::new(),
            password: String::new(),
            db_name: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct MetaStorage {
    connection: MySQLConnection,
}

impl MetaStorage {
    pub async fn new(
        read_only: bool,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        Self::new_with_config(MetaStorageConfig {
            read_only,
            timeout,
            ..MetaStorageConfig::default()
        })
            .await
    }

    pub async fn new_with_config(config: MetaStorageConfig) -> Result<Self> {
        let MetaStorageConfig {
            read_only,
            timeout,
            host,
            port,
            username,
            password,
            db_name,
        } = config;
        let dsn = format!("mysql://{}:{}@{}:{}/{}", username, password, host, port, db_name);
        let connection = MySQLConnection::new(
            dsn.as_str(),
            read_only,
            timeout,
        )
            .await?;

        Ok(Self {
            connection,
        })
    }

    /// Return the available slot that contains a block
    pub async fn get_first_available_block(&self) -> Result<Option<Slot>> {
        debug!("MetaStorage::get_first_available_block request received");

        // inc_new_counter_debug!("storage-mysql-query", 1);
        let mysql = self.connection.client();

        // Use `get_first_key` to get the smallest slot
        let first_block: Option<u64> = mysql
            .get_first_key("sol_mainnet_block", "id")
            .await
            .map_err(|e| Error::StorageBackendError(Box::new(e)))?;

        Ok(first_block.map(|block| block as Slot)) // Convert `u64` to `Slot`
    }

    pub async fn get_slot(&self) -> Result<Option<Slot>> {
        debug!("MetaStorage::get_last_available_block request received");

        // inc_new_counter_debug!("storage-mysql-query", 1);
        let mysql = self.connection.client();

        // Use `get_last_key` to get the largest slot
        let last_block: Option<u64> = mysql
            .get_last_key("sol_mainnet_block", "id")
            .await
            .map_err(|e| Error::StorageBackendError(Box::new(e)))?;

        Ok(last_block.map(|block| block as Slot)) // Convert `u64` to `Slot`
    }

    /// Fetch the next slots after the provided slot that contains a block
    ///
    /// start_slot: slot to start the search from (inclusive)
    /// limit: stop after this many slots have been found
    pub async fn get_confirmed_blocks(&self, start_slot: Slot, limit: usize) -> Result<Vec<Slot>> {
        debug!(
            "MetaStorage::get_confirmed_blocks request received: start_slot = {:?}, limit = {:?}",
            start_slot, limit
        );

        // inc_new_counter_debug!("storage-mysql-query", 1);
        let mysql = self.connection.client();
        let start_key = slot_to_key(start_slot);
        // let end_key = slot_to_key(start_slot + limit as u64);
        let blocks: Vec<u64> = mysql
            .get_row_keys("sol_mainnet_block", Some(&start_key), None, limit as i64)
            .await?;
        Ok(blocks.into_iter().map(|block| block as Slot).collect())
    }

    pub async fn get_block_time(&self, slot: Slot) -> Result<DateTime<Utc>> {
        info!("get_block_time request received");

        let mysql = self.connection.client();
        let key = slot_to_key(slot);

        // Fetch `PrimitiveDateTime` directly from MySQL
        let block_time_primitive: PrimitiveDateTime = mysql
            .get_single_value::<PrimitiveDateTime>("sol_mainnet_block", "block_time", "id", &key)
            .await
            .map_err(|e| match e {
                crate::mysql::Error::RowNotFound => Error::BlockNotFound(slot),
                other => Error::StorageBackendError(Box::new(other)),
            })?;

        // Convert to `DateTime<Utc>` using `DateTime::from_timestamp`
        let block_time = DateTime::<Utc>::from_timestamp(
            block_time_primitive.assume_utc().unix_timestamp(),
            block_time_primitive.assume_utc().microsecond() * 1000,
        )
            .ok_or(Error::BlockNotFound(slot))?;

        Ok(block_time)
    }

    pub async fn get_block_height(&self) -> Result<u64> {
        info!("get_block_height request received");

        debug!("MetaStorage::get_block_height request received to fetch the latest block height");

        // inc_new_counter_debug!("storage-mysql-query", 1);

        let mysql = self.connection.client();

        // Fetch the ID of the latest block
        let latest_block_id: u64 = mysql
            .get_last_key("solana_blocks", "id")
            .await
            .map_err(|e| Error::StorageBackendError(Box::new(e)))?
            .ok_or_else(|| Error::BlockNotFound(0))?; // Handle case where no blocks exist

        debug!("Latest block ID fetched: {}", latest_block_id);

        // Fetch the block height using the latest block ID
        let block_height: u64 = mysql
            .get_single_value::<u64>("solana_blocks", "block_height", "id", &latest_block_id.to_string())
            .await
            .map_err(|e| match e {
                crate::mysql::Error::RowNotFound => Error::BlockNotFound(latest_block_id),
                other => Error::StorageBackendError(Box::new(other)),
            })?;

        debug!("Latest block Height fetched: {}", block_height);

        Ok(block_height)
    }
}
