use {
    crate::{
        request_processor::{JsonRpcConfig},
        rpc_service::JsonRpcService,
    },
    log::*,
    solana_sdk::exit::Exit,
    std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::{Path, PathBuf},
        process::exit,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, RwLock,
        },
    },
};

/// Central struct for managing the RPC server.
pub struct RpcServer {
    config: RpcServerConfig,
    exit: Arc<RwLock<Exit>>,
    json_rpc_service: Option<JsonRpcService>,
    actual_rpc_addr: Option<SocketAddr>,
}

impl RpcServer {
    /// Create a new server with default config
    pub fn new() -> Self {
        Self {
            config: RpcServerConfig::default(),
            exit: Arc::default(),
            json_rpc_service: None,
            actual_rpc_addr: None,
        }
    }

    /// Update the underlying JSON-RPC config
    pub fn with_config(mut self, rpc_config: JsonRpcConfig) -> Self {
        self.config.rpc_config = rpc_config;
        self
    }

    /// Set the desired RPC port
    pub fn with_rpc_port(mut self, port: u16) -> Self {
        self.config.rpc_port = port;
        self
    }

    /// Bind to a specific IP instead of 0.0.0.0
    pub fn with_bind_ip_addr(mut self, ip_addr: IpAddr) -> Self {
        self.config.bind_ip_addr = ip_addr;
        self
    }

    /// Start the server, spawning the JSON-RPC thread
    pub fn start(&mut self, log_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let log_path = Self::init_log_dir(log_path)?;

        let rpc_addr = SocketAddr::new(self.config.bind_ip_addr, self.config.rpc_port);

        info!("Starting RPC server at {}", rpc_addr);

        // Register an exit signal
        let exit_flag = Arc::new(AtomicBool::new(false));
        {
            let exit_flag = exit_flag.clone();
            self.exit
                .write()
                .unwrap()
                .register_exit(Box::new(move || exit_flag.store(true, Ordering::Relaxed)));
        }

        // Start the JSON RPC service
        let json_rpc_service = JsonRpcService::new(
            rpc_addr,
            self.config.rpc_config.clone(),
            &log_path,
            self.exit.clone(),
        )?;

        self.json_rpc_service = Some(json_rpc_service);
        self.actual_rpc_addr = Some(rpc_addr);

        Ok(())
    }

    /// If you need to know the URL from outside
    pub fn rpc_url(&self) -> Option<String> {
        self.actual_rpc_addr
            .map(|addr| format!("http://{}", addr))
    }

    /// Block until the RPC service stops
    pub fn join(mut self) {
        if let Some(service) = self.json_rpc_service.take() {
            service.join().ok();
        }
    }

    /// Validate or create log path
    fn init_log_dir(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        if !path.exists() {
            if let Err(err) = std::fs::create_dir_all(path) {
                eprintln!("Error creating log directory: {path:?}: {err}");
                exit(1);
            }
        }
        Ok(path.to_path_buf())
    }
}

/// Configuration for the server
#[derive(Clone, Debug)]
pub struct RpcServerConfig {
    pub rpc_config: JsonRpcConfig,
    pub rpc_port: u16,
    pub bind_ip_addr: IpAddr,
}

impl Default for RpcServerConfig {
    fn default() -> Self {
        Self {
            rpc_config: JsonRpcConfig::default_for_storage_rpc(),
            rpc_port: 8899, // Default port
            bind_ip_addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }
}