use::{
    log::info,
    serde::Deserialize,
    std::env,
};

const DEFAULT_CONFIG_ENV_KEY: &str = "SVC_CONFIG_PATH";
const CONFIG_PREFIX: &str = "SVC_";

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    /// MySQL host
    pub mysql_host: String,

    /// MySQL port
    pub mysql_port: u16,

    /// MySQL user
    pub mysql_user: String,

    /// MySQL password
    pub mysql_password: String,

    /// MySQL database name
    pub mysql_name: String,
}

impl Config {
    pub fn new() -> Config {
        let filename = match env::var(DEFAULT_CONFIG_ENV_KEY) {
            Ok(filepath) => filepath,
            Err(_) => ".env".into(),
        };
        info!("Trying to read the config file from [{}]", &filename);

        dotenv::from_filename(&filename).ok();
        match envy::prefixed(CONFIG_PREFIX).from_env::<Config>() {
            Ok(config) => config,
            Err(e) => panic!("Config file being read: {}. And error {:?}", &filename, e),
        }
    }
}
