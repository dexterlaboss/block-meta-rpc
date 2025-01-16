use {
    log::*,
    mysql::*,
    mysql::prelude::*,
    std::time::Duration,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O: {0}")]
    Io(std::io::Error),

    #[error("Row not found")]
    RowNotFound,

    #[error("Timeout")]
    Timeout,

    #[error("MySQL")]
    MySQL(mysql::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Self {
        Self::MySQL(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 3306;

#[derive(Debug, Clone)]
pub struct MySQLConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: String,
    pub timeout: Option<Duration>,
}

impl Default for MySQLConfig {
    fn default() -> Self {
        let host = DEFAULT_HOST.to_string();
        let port = DEFAULT_PORT;
        Self {
            host,
            port,
            username: String::new(),
            password: String::new(),
            db_name: String::new(),
            timeout: None,
        }
    }
}

#[derive(Clone)]
pub struct MySQLConnection {
    pool: Pool,
    // timeout: Option<Duration>,
}

impl MySQLConnection {
    pub async fn new(
        url: &str,
        _read_only: bool,
        _timeout: Option<Duration>,
    ) -> Result<Self> {
        info!("Creating MySQL connection");

        let pool = Pool::new(url)?;
        Ok(Self {
            pool,
            // timeout: _timeout,
        })
    }

    pub fn client(&self) -> MySQLClient {
        MySQLClient {
            pool: self.pool.clone(),
            // timeout: self.timeout,
        }
    }
}

pub struct MySQLClient {
    pool: Pool,
    // timeout: Option<Duration>,
}

impl MySQLClient {
    /// Execute a query that returns **all** matching rows.
    /// Synchronous under the hood, but you can call it from async code.
    pub async fn execute_query_all(&self, query: &str) -> Result<Vec<Row>> {
        let mut conn = self.pool.get_conn()?; // Use `get_conn().await` for async
        let rows = conn.query(query)?; // Use `query().await` for async query execution
        Ok(rows)
    }

    /// Execute a query that returns **the first** matching row (if any).
    /// Returns Ok(None) if there are no rows.
    pub async fn execute_query_one(&self, query: &str) -> Result<Option<Row>> {
        let mut conn = self.pool.get_conn()?; // Use `get_conn().await` for async
        let row = conn.exec_first(query, ())?; // Use `exec_first().await` for async query execution
        Ok(row)
    }

    /// Get row keys in lexical order from a table.
    ///
    /// This method demonstrates how we use execute_query_all for multi-row fetches.
    pub async fn get_row_keys<T: FromValue>(
        &self,
        table_name: &str,
        start_at: Option<&str>,
        end_at: Option<&str>,
        rows_limit: i64,
    ) -> Result<Vec<T>> {
        if rows_limit == 0 {
            return Ok(vec![]);
        }

        let mut query = format!("SELECT id FROM {}", table_name);

        if let Some(start) = start_at {
            query.push_str(&format!(" WHERE id >= '{}'", start));
        }

        if let Some(end) = end_at {
            if start_at.is_some() {
                query.push_str(&format!(" AND id <= '{}'", end));
            } else {
                query.push_str(&format!(" WHERE id <= '{}'", end));
            }
        }

        query.push_str(&format!(" LIMIT {}", rows_limit));

        let rows = self.execute_query_all(&query).await?;
        let keys: Vec<T> = rows
            .into_iter()
            .map(|mut row| {
                row.take::<T, _>(0).ok_or(Error::RowNotFound)
            })
            .collect::<Result<_>>()?;

        Ok(keys)
    }

    /// Get the first key of a table based on the given column.
    ///
    /// # Parameters
    /// - `table_name`: Name of the table to query.
    /// - `column_name`: Name of the column to determine the first key.
    ///
    /// Returns the smallest value in the specified column.
    pub async fn get_first_key<T: FromValue>(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Result<Option<T>> {
        let query = format!(
            "SELECT MIN(`{}`) AS first_key FROM `{}`",
            column_name, table_name
        );

        let row_opt = self.execute_query_one(&query).await?;
        if let Some(mut row) = row_opt {
            if let Some(val) = row.take(0) {
                Ok(Some(from_value::<T>(val)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get the last key of a table based on the given column.
    ///
    /// # Parameters
    /// - `table_name`: Name of the table to query.
    /// - `column_name`: Name of the column to determine the last key.
    ///
    /// Returns the largest value in the specified column.
    pub async fn get_last_key<T: FromValue>(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Result<Option<T>> {
        let query = format!(
            "SELECT MAX(`{}`) AS last_key FROM `{}`",
            column_name, table_name
        );

        let row_opt = self.execute_query_one(&query).await?;
        if let Some(mut row) = row_opt {
            if let Some(val) = row.take(0) {
                Ok(Some(from_value::<T>(val)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get a **single row** from a table by specifying the column and the value to match.
    /// Returns Ok(Some(row)) if found, or Ok(None) if no row was found.
    pub async fn get_single_row(
        &self,
        table_name: &str,
        column_to_search: &str,
        value_to_search: &str,
    ) -> Result<Option<Row>> {
        let query = format!(
            "SELECT * FROM {} WHERE {} = '{}' LIMIT 1",
            table_name, column_to_search, value_to_search
        );
        self.execute_query_one(&query).await
    }

    /// Fetch a single column value from a MySQL table.
    ///
    /// # Parameters
    /// - `table_name`: Name of the table to query.
    /// - `field_to_return`: Name of the field to fetch.
    /// - `key_field`: Name of the field to filter rows.
    /// - `key_value`: The value to match in the `key_field`.
    ///
    /// Returns `T` if the query returns a single value matching the criteria.
    pub async fn get_single_value<T: FromValue>(
        &self,
        table_name: &str,
        field_to_return: &str,
        key_field: &str,
        key_value: &str,
    ) -> Result<T> {
        // Form the query dynamically
        let query = format!(
            "SELECT `{}` FROM `{}` WHERE `{}` = '{}' LIMIT 1",
            field_to_return, table_name, key_field, key_value
        );

        // Execute the query and fetch the first row
        let row_opt = self.execute_query_one(&query).await?;
        let mut row = match row_opt {
            None => return Err(Error::RowNotFound), // No rows found
            Some(r) => r,
        };

        // Take the first column's raw `Value`
        let raw_val = row.take(0).ok_or(Error::RowNotFound)?;

        // Convert the `Value` into the requested type `T`
        match from_value_opt(raw_val) {
            Ok(t) => Ok(t),
            Err(_) => Err(Error::RowNotFound), // Conversion failed
        }
    }
}