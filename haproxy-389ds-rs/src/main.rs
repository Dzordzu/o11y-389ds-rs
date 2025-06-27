pub mod cli;
pub mod config;
pub mod haproxy;
pub mod ldap_health;
pub mod web;

use anyhow::Result;
use clap::Parser;
use cli::{ArgFlag, Args};
use config::Config;
use internal::Bind;
use ldap_health::Health;
use std::sync::Arc;
use tokio::{select, sync::Mutex};
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let mut config: Config = if let Some(conf) = &args.config {
        let file = String::from_utf8(std::fs::read(conf)?)?;
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
    tracker.spawn(async move {
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

    tracker.close();
    tracker.wait().await;

    Ok(())
}
