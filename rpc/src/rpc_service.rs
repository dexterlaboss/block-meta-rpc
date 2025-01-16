use {
    crate::{
        rpc::{
            storage_rpc_full::*,
            storage_rpc_minimal::*,
        },
        request_processor::*,
        middleware::RpcRequestMiddleware,
    },
    crossbeam_channel::unbounded,
    jsonrpc_core::{
        MetaIoHandler
    },
    jsonrpc_http_server::{
        hyper, AccessControlAllowOrigin, CloseHandle, DomainsValidation,
        ServerBuilder,
    },
    solana_perf::thread::renice_this_thread,
    solana_sdk::{
        exit::Exit,
    },
    solana_storage_mysql::{
        mysql::{
            MySQLConfig,
        }
    },
    std::{
        net::SocketAddr,
        path::{
            Path,
        },
        sync::{
            Arc, RwLock,
        },
        thread::{self, Builder, JoinHandle},
    },
};

pub struct JsonRpcService {
    thread_hdl: JoinHandle<()>,

    #[cfg(test)]
    pub request_processor: JsonRpcRequestProcessor,

    close_handle: Option<CloseHandle>,
}

impl JsonRpcService {
    pub fn new(
        rpc_addr: SocketAddr,
        config: JsonRpcConfig,
        log_path: &Path,
        rpc_service_exit: Arc<RwLock<Exit>>,
    ) -> Result<Self, String> {
        info!("rpc bound to {:?}", rpc_addr);
        info!("rpc configuration: {:?}", config);
        let rpc_threads = 1.max(config.rpc_threads);
        let rpc_niceness_adj = config.rpc_niceness_adj;

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(rpc_threads)
                .on_thread_start(move || renice_this_thread(rpc_niceness_adj).unwrap())
                .thread_name("solRpcEl")
                .enable_all()
                .build()
                .expect("Runtime"),
        );

        let mysql_metadata_storage =
            if let Some(MySQLConfig {
                            ref host,
                            ref port,
                            ref username,
                            ref password,
                            ref db_name,
                            timeout,
                        }) = config.rpc_mysql_config
            {
                let mysql_config = solana_storage_mysql::meta_storage::MetaStorageConfig {
                    read_only: true,
                    timeout,
                    host: host.clone(),
                    port: port.clone(),
                    username: username.clone(),
                    password: password.clone(),
                    db_name: db_name.clone(),
                };
                runtime
                    .block_on(solana_storage_mysql::meta_storage::MetaStorage::new_with_config(mysql_config))
                    .map(|mysql_metadata_storage| {
                        info!("MySQL metadata storage initialized");
                        Some(mysql_metadata_storage)
                    })
                    .unwrap_or_else(|err| {
                        error!("Failed to initialize MySQL metadata storage: {:?}", err);
                        None
                    })
            } else {
                None
            };

        let full_api = config.full_api;
        let max_request_body_size = config
            .max_request_body_size
            .unwrap_or(MAX_REQUEST_BODY_SIZE);
        let request_processor = JsonRpcRequestProcessor::new(
            config,
            rpc_service_exit.clone(),
            mysql_metadata_storage,
        );

        #[cfg(test)]
            let test_request_processor = request_processor.clone();

        let log_path = log_path.to_path_buf();

        let (close_handle_sender, close_handle_receiver) = unbounded();
        let thread_hdl = Builder::new()
            .name("solJsonRpcSvc".to_string())
            .spawn(move || {
                renice_this_thread(rpc_niceness_adj).unwrap();

                let mut io = MetaIoHandler::default();

                io.extend_with(MinimalImpl.to_delegate());
                if full_api {
                    io.extend_with(FullImpl.to_delegate());
                }

                let request_middleware = RpcRequestMiddleware::new(
                    log_path,
                );
                let server = ServerBuilder::with_meta_extractor(
                    io,
                    move |_req: &hyper::Request<hyper::Body>| request_processor.clone(),
                )
                    .event_loop_executor(runtime.handle().clone())
                    .threads(1)
                    .cors(DomainsValidation::AllowOnly(vec![
                        AccessControlAllowOrigin::Any,
                    ]))
                    .cors_max_age(86400)
                    .request_middleware(request_middleware)
                    .max_request_body_size(max_request_body_size)
                    .start_http(&rpc_addr);

                if let Err(e) = server {
                    warn!(
                        "JSON RPC service unavailable error: {:?}. \n\
                           Also, check that port {} is not already in use by another application",
                        e,
                        rpc_addr.port()
                    );
                    close_handle_sender.send(Err(e.to_string())).unwrap();
                    return;
                }

                let server = server.unwrap();
                close_handle_sender.send(Ok(server.close_handle())).unwrap();
                server.wait();
            })
            .unwrap();

        let close_handle = close_handle_receiver.recv().unwrap()?;
        let close_handle_ = close_handle.clone();
        rpc_service_exit
            .write()
            .unwrap()
            .register_exit(Box::new(move || close_handle_.close()));
        Ok(Self {
            thread_hdl,
            #[cfg(test)]
            request_processor: test_request_processor,
            close_handle: Some(close_handle),
        })
    }

    pub fn exit(&mut self) {
        if let Some(c) = self.close_handle.take() {
            c.close()
        }
    }

    pub fn join(self) -> thread::Result<()> {
        self.thread_hdl.join()
    }
}

