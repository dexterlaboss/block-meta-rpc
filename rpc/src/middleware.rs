use {
    jsonrpc_http_server::{
        hyper,
        RequestMiddleware,
        RequestMiddlewareAction,
    },
    std::{
        path::{
            PathBuf
        },
    },
};

pub struct RpcRequestMiddleware {
    // log_path: PathBuf,
}

impl RpcRequestMiddleware {
    pub fn new(
        _log_path: PathBuf,
    ) -> Self {
        Self {
            // log_path,
        }
    }

    #[allow(dead_code)]
    fn internal_server_error() -> hyper::Response<hyper::Body> {
        hyper::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(hyper::Body::empty())
            .unwrap()
    }

    fn health_check(&self) -> &'static str {
        let response = "ok";
        info!("health check: {}", response);
        response
    }
}

impl RequestMiddleware for RpcRequestMiddleware {
    fn on_request(&self, request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
        trace!("request uri: {}", request.uri());

        if let Some(result) = process_rest(request.uri().path()) {
            hyper::Response::builder()
                .status(hyper::StatusCode::OK)
                .body(hyper::Body::from(result))
                .unwrap()
                .into()
        } else if request.uri().path() == "/health" {
            hyper::Response::builder()
                .status(hyper::StatusCode::OK)
                .body(hyper::Body::from(self.health_check()))
                .unwrap()
                .into()
        } else {
            request.into()
        }
    }
}

fn process_rest(path: &str) -> Option<String> {
    match path {
        //
        // Add custom url endpoints here
        //
        _ => None,
    }
}