use {
    crate::{
        request_processor::MAX_REQUEST_BODY_SIZE,
    },
    clap::{
        App,
        Arg,
        ArgMatches,
    },
    log::warn,
    solana_clap_utils::{
        input_validators::{
            is_parsable,
        },
    },
    solana_sdk::{
        rpc_port,
    },
    solana_perf::{
        thread::{
            is_niceness_adjustment_valid,
        }
    },
};

/// Deprecated argument description should be moved into the [`deprecated_arguments()`] function,
/// expressed as an instance of this type.
struct DeprecatedArg {
    /// Deprecated argument description, moved here as is.
    ///
    /// `hidden` property will be modified by [`deprecated_arguments()`] to only show this argument
    /// if [`hidden_unless_forced()`] says they should be displayed.
    arg: Arg<'static, 'static>,

    /// If simply replaced by a different argument, this is the name of the replacement.
    ///
    /// Content should be an argument name, as presented to users.
    replaced_by: Option<&'static str>,

    /// An explanation to be shown to the user if they still use this argument.
    ///
    /// Content should be a complete sentence or several, ending with a period.
    usage_warning: Option<&'static str>,
}

fn deprecated_arguments() -> Vec<DeprecatedArg> {
    let mut res = vec![];

    // This macro reduces indentation and removes some noise from the argument declaration list.
    macro_rules! add_arg {
        (
            $arg:expr
            $( , replaced_by: $replaced_by:expr )?
            $( , usage_warning: $usage_warning:expr )?
            $(,)?
        ) => {
            let replaced_by = add_arg!(@into-option $( $replaced_by )?);
            let usage_warning = add_arg!(@into-option $( $usage_warning )?);
            res.push(DeprecatedArg {
                arg: $arg,
                replaced_by,
                usage_warning,
            });
        };

        (@into-option) => { None };
        (@into-option $v:expr) => { Some($v) };
    }

    add_arg!(Arg::with_name("minimal_rpc_api")
        .long("minimal-rpc-api")
        .takes_value(false)
        .help("Only expose the RPC methods required to serve snapshots to other nodes"));

    res
}

pub fn warn_for_deprecated_arguments(matches: &ArgMatches) {
    for DeprecatedArg {
        arg,
        replaced_by,
        usage_warning,
    } in deprecated_arguments().into_iter()
    {
        if matches.is_present(arg.b.name) {
            let mut msg = format!("--{} is deprecated", arg.b.name.replace('_', "-"));
            if let Some(replaced_by) = replaced_by {
                msg.push_str(&format!(", please use --{replaced_by}"));
            }
            msg.push('.');
            if let Some(usage_warning) = usage_warning {
                msg.push_str(&format!("  {usage_warning}"));
                if !msg.ends_with('.') {
                    msg.push('.');
                }
            }
            warn!("{}", msg);
        }
    }
}

pub fn port_validator(port: String) -> Result<(), String> {
    port.parse::<u16>()
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

pub fn storage_rpc_service<'a>(version: &'a str, default_args: &'a DefaultStorageRpcArgs) -> App<'a, 'a> {
    return App::new("solana-storage-rpc")
        .about("Solana Storage RPC Service")
        .version(version)
        .arg(
            Arg::with_name("log_path")
                .short("l")
                .long("log-path")
                .value_name("DIR")
                .takes_value(true)
                .required(true)
                .default_value("log")
                .help("Use DIR as log location"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .takes_value(false)
                .conflicts_with("log")
                .help("Quiet mode: suppress normal output"),
        )
        .arg(
            Arg::with_name("log")
                .long("log")
                .takes_value(false)
                .conflicts_with("quiet")
                .help("Log mode: stream the launcher log"),
        )
        .arg(
            Arg::with_name("rpc_port")
                .long("rpc-port")
                .value_name("PORT")
                .takes_value(true)
                .default_value(&default_args.rpc_port)
                .validator(port_validator)
                .help("Port for the RPC service"),
        )
        .arg(
            Arg::with_name("enable_rpc_mysql_meta_storage")
                .long("enable-rpc-mysql-meta-storage")
                .takes_value(false)
                .hidden(true)
                .help("Fetch block metadata info from MySQL instance"),
        )
        .arg(
            Arg::with_name("rpc_mysql_address")
                .long("rpc-mysql-address")
                .value_name("ADDRESS")
                .takes_value(true)
                .hidden(true)
                .default_value("127.0.0.1:9090")
                .help("Address of MySQL instance to use"),
        )
        .arg(
            Arg::with_name("rpc_mysql_timeout")
                .long("rpc-mysql-timeout")
                .value_name("SECONDS")
                .validator(is_parsable::<u64>)
                .takes_value(true)
                .default_value(&default_args.rpc_mysql_timeout)
                .help("Number of seconds before timing out RPC requests backed by MySQL"),
        )
        .arg(
            Arg::with_name("bind_address")
                .long("bind-address")
                .value_name("HOST")
                .takes_value(true)
                .validator(solana_net_utils::is_host)
                .default_value("0.0.0.0")
                .help("IP address to bind the rpc service [default: 0.0.0.0]"),
        )
        .arg(
            Arg::with_name("rpc_threads")
                .long("rpc-threads")
                .value_name("NUMBER")
                .validator(is_parsable::<usize>)
                .takes_value(true)
                .default_value(&default_args.rpc_threads)
                .help("Number of threads to use for servicing RPC requests"),
        )
        .arg(
            Arg::with_name("rpc_niceness_adj")
                .long("rpc-niceness-adjustment")
                .value_name("ADJUSTMENT")
                .takes_value(true)
                .validator(is_niceness_adjustment_valid)
                .default_value(&default_args.rpc_niceness_adjustment)
                .help("Add this value to niceness of RPC threads. Negative value \
                      increases priority, positive value decreases priority.")
        )
        .arg(
            Arg::with_name("rpc_max_request_body_size")
                .long("rpc-max-request-body-size")
                .value_name("BYTES")
                .takes_value(true)
                .validator(is_parsable::<usize>)
                .default_value(&default_args.rpc_max_request_body_size)
                .help("The maximum request body size accepted by rpc service"),
        )
        .arg(
            Arg::with_name("log_messages_bytes_limit")
                .long("log-messages-bytes-limit")
                .value_name("BYTES")
                .validator(is_parsable::<usize>)
                .takes_value(true)
                .help("Maximum number of bytes written to the program log before truncation")
        )
    ;
}

pub struct DefaultStorageRpcArgs {
    pub rpc_port: String,
    pub rpc_mysql_timeout: String,
    pub rpc_threads: String,
    pub rpc_niceness_adjustment: String,
    pub rpc_max_request_body_size: String,
    pub enable_rpc_mysql_meta_storage: bool,
}

impl DefaultStorageRpcArgs {
    pub fn new() -> Self {
        DefaultStorageRpcArgs {
            rpc_port: rpc_port::DEFAULT_RPC_PORT.to_string(),
            rpc_mysql_timeout: "5".to_string(),
            rpc_threads: num_cpus::get().to_string(),
            rpc_niceness_adjustment: "0".to_string(),
            rpc_max_request_body_size: MAX_REQUEST_BODY_SIZE.to_string(),
            enable_rpc_mysql_meta_storage: true,
        }
    }
}

impl Default for DefaultStorageRpcArgs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn make_sure_deprecated_arguments_are_sorted_alphabetically() {
        let deprecated = deprecated_arguments();

        for i in 0..deprecated.len().saturating_sub(1) {
            let curr_name = deprecated[i].arg.b.name;
            let next_name = deprecated[i + 1].arg.b.name;

            assert!(
                curr_name != next_name,
                "Arguments in `deprecated_arguments()` should be distinct.\n\
                 Arguments {} and {} use the same name: {}",
                i,
                i + 1,
                curr_name,
            );

            assert!(
                curr_name < next_name,
                "To generate better diffs and for readability purposes, `deprecated_arguments()` \
                 should list arguments in alphabetical order.\n\
                 Arguments {} and {} are not.\n\
                 Argument {} name: {}\n\
                 Argument {} name: {}",
                i,
                i + 1,
                i,
                curr_name,
                i + 1,
                next_name,
            );
        }
    }
}
