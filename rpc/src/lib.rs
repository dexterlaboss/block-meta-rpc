#![allow(clippy::integer_arithmetic)]
#![recursion_limit = "2048"]

pub mod custom_error;

pub mod request_processor;
pub mod rpc_service;

pub mod rpc;

pub mod cli;
pub mod config;

pub mod logging;

pub mod rpc_server;

pub mod middleware;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate serde_json;

// #[macro_use]
// extern crate solana_metrics;

#[cfg(test)]
#[macro_use]
extern crate matches;
