use {
    clap::value_t_or_exit,
    log::*,
    solana_net_utils::parse_host,
    block_meta_rpc::{
        cli,
        config::Config,
        logging::redirect_stderr_to_file,
        request_processor::JsonRpcConfig,
        rpc_server::RpcServer,
    },
    solana_storage_mysql::{
        mysql::MySQLConfig,
    },
    solana_version::version,
    std::{
        fs,
        path::PathBuf,
        process::exit,
        sync::Arc,
        time::{
            SystemTime,
            UNIX_EPOCH,
        },
    },
    symlink,
};

#[derive(PartialEq, Eq)]
enum Output {
    None,
    Log,
}

fn main() {
    let default_args = cli::DefaultStorageRpcArgs::new();
    let version = version!(); // Store version in a variable
    let matches = cli::storage_rpc_service(version, &default_args).get_matches();

    // Decide logging style
    let output = if matches.is_present("quiet") {
        Output::None
    } else {
        Output::Log
    };

    // Create the log directory if necessary
    let log_path = value_t_or_exit!(matches, "log_path", PathBuf);
    if !log_path.exists() {
        fs::create_dir(&log_path).unwrap_or_else(|err| {
            eprintln!(
                "Error: Unable to create directory {}: {}",
                log_path.display(),
                err
            );
            exit(1);
        });
    }

    // Possibly redirect logs to a symlinked file
    let rpc_service_log_symlink = log_path.join("service.log");
    let logfile = if output != Output::Log {
        let timestamped_log = format!(
            "service-{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let _ = fs::remove_file(&rpc_service_log_symlink);
        symlink::symlink_file(&timestamped_log, &rpc_service_log_symlink).unwrap();
        Some(
            log_path
                .join(timestamped_log)
                .into_os_string()
                .into_string()
                .unwrap(),
        )
    } else {
        None
    };

    // Set up the logger
    let _logger_thread = redirect_stderr_to_file(logfile);

    info!("solana-meta-rpc {}", version);
    info!(
        "Starting block metadata rpc service with: {:#?}",
        std::env::args_os()
    );

    // Grab CLI parameters
    let rpc_port = value_t_or_exit!(matches, "rpc_port", u16);
    let bind_address = matches.value_of("bind_address").map(|bind_address| {
        parse_host(bind_address).unwrap_or_else(|err| {
            eprintln!("Failed to parse --bind-address: {err}");
            exit(1);
        })
    });

    let app_config = Arc::new(Config::new());

    // Prepare JSON RPC config
    let rpc_mysql_config = Some(MySQLConfig {
        host: app_config.mysql_host.clone(),
        port: app_config.mysql_port,
        username: app_config.mysql_user.clone(),
        password: app_config.mysql_password.clone(),
        db_name: app_config.mysql_name.clone(),
        timeout: None,
    });

    let mut rpc_config = JsonRpcConfig::default_for_storage_rpc();
    rpc_config.rpc_mysql_config = rpc_mysql_config;
    rpc_config.obsolete_v1_7_api = matches.is_present("obsolete_v1_7_rpc_api");
    rpc_config.rpc_threads = value_t_or_exit!(matches, "rpc_threads", usize);
    rpc_config.rpc_niceness_adj = value_t_or_exit!(matches, "rpc_niceness_adj", i8);
    rpc_config.max_request_body_size = Some(value_t_or_exit!(
        matches,
        "rpc_max_request_body_size",
        usize
    ));

    // Build and start the RPC server
    let mut rpc_server = RpcServer::new()
        .with_config(rpc_config)
        .with_rpc_port(rpc_port);

    if let Some(ip_addr) = bind_address {
        rpc_server = rpc_server.with_bind_ip_addr(ip_addr);
    }

    if let Err(err) = rpc_server.start(&log_path) {
        eprintln!("Error: failed to start block metadata rpc service: {err}");
        exit(1);
    }

    // The server is running at this point
    rpc_server.join();
}