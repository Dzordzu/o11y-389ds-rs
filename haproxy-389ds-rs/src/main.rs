pub mod cli;
pub mod config;
pub mod haproxy;
pub mod ldap_health;
pub mod web;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{ArgFlag, Args};
use config::Config;
use internal::{Bind, query::CustomQuery};
use ldap_health::Health;
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
    sync::Mutex,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub struct AppStateBase {
    pub health: Health,
    pub config: config::Config,
    pub current_reponse: haproxy::Response,
}

impl AppStateBase {
    pub fn new(config: config::Config) -> Self {
        AppStateBase {
            health: Health::new(),
            current_reponse: haproxy::Response::new_up(),
            config,
        }
    }

    pub fn evaluate(&mut self) {
        self.health.evaluate(&mut self.current_reponse);
    }
}

pub async fn accessibility_loop(
    config: Config,
    app_state: AppState,
    cancel_token: CancellationToken,
) {
    tracing::info!("Starting 389ds accessibility checks");

    loop {
        if let Err(error) = check_ldap_connection(&config).await {
            tracing::error!("Error: {}", error);
            app_state.lock().await.health.status.is_reachable = false;
        } else {
            app_state.lock().await.health.status.is_reachable = true;
        }

        select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                config.haproxy.scrape_interval_seconds.ldap_accessibility,
            )) => {

            },
            _ = cancel_token.cancelled() => {
                break
            }
        }
    }
}

pub async fn systemd_status_loop(
    config: Config,
    app_state: AppState,
    cancel_token: CancellationToken,
) -> Result<()> {
    tracing::info!("Starting systemd status checks");

    loop {
        let instance = config.common.scrapers.dsctl.instance_name.clone();
        let timeout_seconds = config.common.scrapers.dsctl.timeout_seconds;
        let cli_config = internal::cli::CommandConfig::new(timeout_seconds, instance);

        match cli_config.systemd_running().await {
            Err(error) => {
                tracing::error!("Error: {}", error);
                app_state.lock().await.health.status.is_systemd_running = false;
            }
            Ok(x) => {
                if x {
                    app_state.lock().await.health.status.is_systemd_running = true;
                } else {
                    tracing::error!("Systemd is not running");
                    app_state.lock().await.health.status.is_systemd_running = false;
                }
            }
        }

        select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                config.haproxy.scrape_interval_seconds.ldap_accessibility,
            )) => {

            },
            _ = cancel_token.cancelled() => {
                break
            }
        }
    }

    Ok(())
}

pub type AppState = Arc<Mutex<AppStateBase>>;

pub async fn check_ldap_connection(config: &config::Config) -> Result<()> {
    config.common.ldap_config.connect().await?;
    Ok(())
}

pub async fn handle_query(
    mut query: CustomQuery,
    haproxy_query: &config::HaproxyQuery,
) -> Result<bool> {
    if let config::HaproxyQuery::CountAttrs(count_entries) = haproxy_query {
        query.attrs = vec![count_entries.attr.clone()];
    }

    let metrics = query.get_metrics().await?;

    match haproxy_query {
        config::HaproxyQuery::CountEntries(counter_haproxy_query) => {
            let value = metrics.object_count;

            if let Some(less_than) = counter_haproxy_query.less_than {
                if value >= less_than {
                    return Ok(false);
                }
            }

            if let Some(greater_than) = counter_haproxy_query.greater_than {
                if value <= greater_than {
                    return Ok(false);
                }
            }
        }
        config::HaproxyQuery::CountAttrs(counter_haproxy_query) => {
            let value = metrics.attrs_count;

            if let Some(less_than) = counter_haproxy_query.counter.less_than {
                if value >= less_than {
                    return Ok(false);
                }
            }

            if let Some(greater_than) = counter_haproxy_query.counter.greater_than {
                if value <= greater_than {
                    return Ok(false);
                }
            }
        }
        config::HaproxyQuery::Success(_) => {
            // query executed, we are happy
        }
    }

    Ok(true)
}

#[derive(Debug, Clone)]
struct SetupQueriesTrio {
    named_check: String,
    haproxy_query: config::HaproxyQuery,
    query_definition: internal::query::CustomQuery,
}

pub async fn setup_queries_loops(
    config: Config,
    app_state: AppState,
    cancel_token: CancellationToken,
    tracker: &TaskTracker,
) {
    tracing::info!("Setting up 389ds queries");

    for mut trio in config
        .haproxy
        .query
        .iter()
        .filter_map(|(named_check, haproxy_query)| {
            if let Some(query_def) = config
                .common
                .scrapers
                .query
                .iter()
                .find(|query| query.name == haproxy_query.name())
            {
                Some(SetupQueriesTrio {
                    named_check: named_check.clone(),
                    haproxy_query: haproxy_query.clone(),
                    query_definition: query_def.clone(),
                })
            } else {
                tracing::error!(
                    "Query {} not found in scrapers definitions",
                    haproxy_query.name()
                );
                None
            }
        })
    {
        let cancel_token = cancel_token.clone();
        let config = config.clone();

        if let Some(max_entries) = trio.haproxy_query.max_entries() {
            trio.query_definition.max_entries = Some(max_entries as i32);
        }

        let app_state = app_state.clone();
        tracker.spawn(async move {
            trio.query_definition.ldap_config = Some(config.common.ldap_config.clone());
            let query_name = trio.named_check;
            loop {
                match handle_query(trio.query_definition.clone(), &trio.haproxy_query).await {
                    Err(e) => {
                        tracing::error!(
                            "Error executing query {} (scrape name: {}): {}",
                            query_name,
                            trio.haproxy_query.name(),
                            e
                        );
                        app_state
                            .lock()
                            .await
                            .health
                            .status
                            .queries_status
                            .insert(query_name.to_string(), false);
                    }
                    Ok(x) => {
                        app_state
                            .lock()
                            .await
                            .health
                            .status
                            .queries_status
                            .insert(query_name.to_string(), x);
                    }
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        trio.haproxy_query.scrape_interval_seconds().unwrap_or(
                            config.haproxy.scrape_interval_seconds.query
                        ),
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        });
    }
}

async fn read_until_newline(stream: &mut TcpStream) -> Result<String> {
    let mut bytes: Vec<u8> = vec![];

    for _ in 0..10000 {
        let byte = stream.read_u8().await.context(format!(
            "Could not byte from the message. Currently processed {} bytes",
            bytes.len()
        ))?;

        if byte == b'\n' {
            return Ok(String::from_utf8(bytes)?);
        }

        bytes.push(byte);
    }

    Err(anyhow::anyhow!(
        "Message larger than 10000. Cowardly exiting"
    ))
}

async fn process_stream(mut stream: TcpStream, app_state: AppState) -> Result<()> {
    let command = read_until_newline(&mut stream).await?;
    if command == "ping" {
        let mut data = app_state.lock().await;
        data.evaluate();
        let response = data.current_reponse.to_haproxy_string();

        stream.writable().await.context("Could not wait to write")?;
        stream
            .write(&response.into_bytes().to_vec())
            .await
            .context("Failed to send reponse")?;
    } else {
        return Err(anyhow::anyhow!("Unknown command: {}", command));
    }

    Ok(())
}

async fn tcp_server_loop(
    config: Config,
    app_state: AppState,
    _cancel_token: CancellationToken,
) -> Result<()> {
    let addr = format!(
        "{}:{}",
        config.haproxy.expose_address, config.haproxy.expose_tcp_port
    );
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Starting tcp server. Listening on {}", &addr);

    loop {
        let app_state = app_state.clone();
        if let Err(e) = {
            let (socket, _) = listener.accept().await?;
            process_stream(socket, app_state).await
        } {
            tracing::error!("Error during tcp processing {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let mut config: Config = if let Some(conf) = &args.config {
        let file = String::from_utf8(
            std::fs::read(conf).context(format!("Could not read config file: {conf:?}"))?,
        )?;
        toml::from_str(&file)?
    } else {
        Default::default()
    };

    if let Some(page_size) = args.page_size {
        config.common.ldap_config.page_size = page_size;
    }

    if let Some(dn) = args.binddn {
        let pass = args.bindpass.unwrap();
        let bind = Bind { dn, pass };
        config.common.ldap_config.bind = Some(bind);
    }

    if let Some(host) = args.host {
        config.common.ldap_config.uri = host;
    }

    if let Some(expose_address) = args.expose_address {
        config.haproxy.expose_address = expose_address;
    }

    if let Some(expose_port) = args.expose_port {
        config.haproxy.expose_port = expose_port;
    }

    if let Some(basedn) = args.basedn {
        config.common.ldap_config.default_base = basedn;
    }

    if args.skip_cert_verification {
        config.common.ldap_config.verify_certs = false;
    }

    if config.common.ldap_config.default_base.is_empty() {
        config.common.ldap_config.detect_base().await?;
        tracing::info!("Set base to the {}", config.common.ldap_config.default_base);
    }

    for disable_flag in args.disable_flags {
        match disable_flag {
            ArgFlag::Replication => config.haproxy.scrape_flags.replication_status = false,
            ArgFlag::LdapMonitor => config.haproxy.scrape_flags.ldap_monitoring = false,
        }
    }

    for enable_flags in args.enable_flags {
        match enable_flags {
            ArgFlag::Replication => config.haproxy.scrape_flags.replication_status = true,
            ArgFlag::LdapMonitor => config.haproxy.scrape_flags.ldap_monitoring = true,
        }
    }

    let tracker = TaskTracker::new();
    let cancel_token_orig = CancellationToken::new();
    let app_state: AppState = Arc::new(Mutex::new(AppStateBase::new(config.clone())));

    let app_state_clone = app_state.clone();
    let config_clone = config.clone();
    let webserver_loop = tracker.spawn(async move {
        tracing::info!("Starting webserver");
        web::webserver(
            config_clone.haproxy.expose_address,
            config_clone.haproxy.expose_port,
            app_state_clone,
        )
        .await
    });

    let app_state_clone = app_state.clone();
    let config_clone = config.clone();
    let cancel_token = cancel_token_orig.clone();
    tracker.spawn(
        async move { accessibility_loop(config_clone, app_state_clone, cancel_token).await },
    );

    let app_state_clone = app_state.clone();
    let config_clone = config.clone();
    let cancel_token = cancel_token_orig.clone();
    if config.haproxy.scrape_flags.systemd_status {
        tracker.spawn(async move {
            systemd_status_loop(config_clone, app_state_clone, cancel_token).await
        });
    } else {
        tracing::info!("Skipping systemd status checks");
        app_state_clone
            .lock()
            .await
            .health
            .status
            .is_systemd_running = true;
    }

    let app_state_clone = app_state.clone();
    let config_clone = config.clone();
    let cancel_token = cancel_token_orig.clone();
    setup_queries_loops(config_clone, app_state_clone, cancel_token, &tracker).await;

    let config_clone = config.clone();
    let cancel_token = cancel_token_orig.clone();
    let app_state_clone = app_state.clone();
    tracker
        .spawn(async move { tcp_server_loop(config_clone, app_state_clone, cancel_token).await });

    tracing::info!("Awaiting close of the webserver_loop");
    webserver_loop.await?;

    tracker.close();
    tracker.wait().await;

    Ok(())
}
