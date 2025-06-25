pub mod monitor;
pub mod replica;

use std::{net::SocketAddr, time::Instant};

use crate::monitor::{get_ldap_metrics, MetricsCommonData};
use anyhow::Result;
use clap::{ArgGroup, Parser};
use internal::{cli::CommandConfig, query::CustomQuery, Bind, LdapConfig};
use metrics::{counter, describe_counter, describe_gauge, gauge};
use metrics_exporter_prometheus::PrometheusBuilder;
use replica::{get_ldap_replica_metrics, ReplicationCommonData};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::select;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[derive(Default)]
pub struct DsctlCommonData {
    /// DSLE of the all known healthchecks
    pub healthchecks: HashSet<internal::cli::HealthcheckEntry>,
}

async fn get_dsctl_metrics(
    cmd_cfg: &CommandConfig,
    common_data: &mut DsctlCommonData,
) -> Result<()> {
    let healthchecks = cmd_cfg.healthchecks().await?;

    let g = gauge!("dsctl.healthcheck.healthy", "instance" => cmd_cfg.instance_name.clone());
    g.set((healthchecks.is_empty()) as u8 as f64);

    let healthcheck_names = healthchecks
        .iter()
        .map(|x| x.dsle.clone())
        .collect::<Vec<String>>();

    for outdated_check in common_data
        .healthchecks
        .iter()
        .filter(|check| !healthcheck_names.contains(&check.dsle))
    {
        let g = gauge!(
            "dsctl.healthcheck.error",
            "instance" => cmd_cfg.instance_name.clone(),
            "severity" => outdated_check.severity.to_string(),
            "dsle" => outdated_check.dsle.clone()
        );
        g.set(0_f64);
    }

    for healthcheck in healthchecks {
        let g = gauge!(
            "dsctl.healthcheck.error",
            "instance" => cmd_cfg.instance_name.clone(),
            "severity" => healthcheck.severity.to_string(),
            "dsle" => healthcheck.dsle.clone()
        );
        g.set(1_f64);

        // Insert to the common data
        common_data.healthchecks.insert(healthcheck);
    }

    Ok(())
}

pub async fn get_gids_metrics(ldap_config: &LdapConfig) -> Result<()> {
    const PREFIX: &str = "query.gids.";

    for (account, number) in internal::gids::missing_gids_to_uid_mapping(ldap_config).await? {
        let account = account.to_string();
        let gauge = gauge!(format!("{PREFIX}unresolvable_count"), "gid" => account);
        gauge.set(number as f64);
    }

    Ok(())
}

fn default_true() -> bool {
    true
}

fn default_scrape_interval_seconds() -> u64 {
    5
}

fn default_expose_port() -> u16 {
    9100
}

fn default_expose_address() -> String {
    "0.0.0.0".to_string()
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExporterQuery {
    name: String,

    #[serde(default = "default_scrape_interval_seconds")]
    scrape_interval_seconds: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExporterConfig {
    #[serde(default = "default_expose_port")]
    pub expose_port: u16,

    #[serde(default = "default_expose_address")]
    pub expose_address: String,

    #[serde(default = "default_scrape_interval_seconds")]
    pub scrape_interval_seconds: u64,

    #[serde(default)]
    pub scrape_flags: ScrapeFlags,

    #[serde(default)]
    pub queries: Vec<ExporterQuery>,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            expose_port: default_expose_port(),
            expose_address: default_expose_address(),
            scrape_interval_seconds: default_scrape_interval_seconds(),
            scrape_flags: Default::default(),
            queries: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub exporter: ExporterConfig,

    #[serde(flatten)]
    pub common: internal::config::CommonConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScrapeFlags {
    #[serde(default = "default_true")]
    /// Use cn=monitor to gather metrics
    pub ldap_monitoring: bool,

    #[serde(default = "default_true")]
    /// Check replication status using ldapsearch
    pub replication_status: bool,

    #[serde(default)]
    /// Count unresolvable primary gids of posixUser; count low number gids
    pub gids_info: bool,

    #[serde(default)]
    /// Run dsctl healthcheck
    pub dsctl: bool,
}

impl Default for ScrapeFlags {
    fn default() -> Self {
        Self {
            ldap_monitoring: true,
            replication_status: true,
            gids_info: false,
            dsctl: false,
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ArgFlag {
    /// Parse replication entries
    Replication,

    /// Parse monitoring entry
    LdapMonitor,

    /// Count unresolvable primary gids of posixUser
    GidsInfo,

    /// Run dsctl commands. For example dsctl healthcheck
    Dsctl,
}

#[derive(Parser)]
#[clap(group(ArgGroup::new("bind").requires_all(["binddn", "bindpass"]).multiple(true)))]
pub struct Args {
    /// Path to the TOML configuration file
    #[clap(short, long)]
    config: Option<PathBuf>,

    /// LDAP paging setting
    #[clap(short = 'P', long)]
    page_size: Option<i32>,

    /// Disable TLS cert verification
    #[clap(short = 'C', long, default_value_t = false)]
    skip_cert_verification: bool,

    #[clap(short = 'a', long)]
    expose_address: Option<String>,

    #[clap(short = 'p', long)]
    expose_port: Option<u16>,

    #[clap(short = 'b', long)]
    basedn: Option<String>,

    #[clap(short = 'D', long)]
    #[clap(group = "bind")]
    binddn: Option<String>,

    #[clap(short = 'w', long)]
    #[clap(group = "bind")]
    bindpass: Option<String>,

    #[clap(short = 'H', long)]
    host: Option<String>,

    #[clap(short = 'I', long)]
    scrape_interval_seconds: Option<u64>,

    #[clap(short = 'e', long)]
    #[clap(value_enum)]
    enable_flags: Vec<ArgFlag>,

    #[clap(short = 'd', long)]
    #[clap(value_enum)]
    disable_flags: Vec<ArgFlag>,
}

async fn setup_query_checks(
    cancel_token: CancellationToken,
    config: Config,
    tracker: &TaskTracker,
) -> Result<()> {
    let queries = config.exporter.queries.iter().filter_map(|exporter_query| {
        if let Some(query_def) = config
            .common
            .scrapers
            .query
            .iter()
            .find(|query| query.name == exporter_query.name)
        {
            Some((exporter_query.clone(), query_def.clone()))
        } else {
            tracing::error!("Query {} not found", exporter_query.name);
            let health_gauge =
                gauge!("internal.health.query_not_found", "name" => exporter_query.name.clone());
            health_gauge.set(0);
            None
        }
    });

    describe_gauge!("internal.health.query", "queries scraper status");

    for mut query in queries {
        let cancel_token = cancel_token.clone();
        let config = config.clone();

        tracker.spawn(async move {
            query.1.ldap_config = config.common.ldap_config.clone();
            let health_gauge = gauge!("internal.health.query", "name" => query.1.name.clone());

            loop {
                if let Err(e) = handle_query(query.1.clone()).await {
                    tracing::error!("Error: {}", e);
                    health_gauge.set(0);
                } else {
                    health_gauge.set(1);
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        query.0.scrape_interval_seconds,
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        });
    }

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

    if let Some(scrape_interval_seconds) = args.scrape_interval_seconds {
        config.exporter.scrape_interval_seconds = scrape_interval_seconds;
    }

    if let Some(expose_address) = args.expose_address {
        config.exporter.expose_address = expose_address;
    }

    if let Some(expose_port) = args.expose_port {
        config.exporter.expose_port = expose_port;
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
            ArgFlag::Replication => config.exporter.scrape_flags.replication_status = false,
            ArgFlag::LdapMonitor => config.exporter.scrape_flags.ldap_monitoring = false,
            ArgFlag::GidsInfo => config.exporter.scrape_flags.gids_info = false,
            ArgFlag::Dsctl => config.exporter.scrape_flags.dsctl = false,
        }
    }

    for enable_flags in args.enable_flags {
        match enable_flags {
            ArgFlag::Replication => config.exporter.scrape_flags.replication_status = true,
            ArgFlag::LdapMonitor => config.exporter.scrape_flags.ldap_monitoring = true,
            ArgFlag::GidsInfo => config.exporter.scrape_flags.gids_info = true,
            ArgFlag::Dsctl => config.exporter.scrape_flags.dsctl = true,
        }
    }

    let builder = PrometheusBuilder::new()
        .with_http_listener(
            format!(
                "{}:{}",
                config.exporter.expose_address, config.exporter.expose_port
            )
            .parse::<SocketAddr>()?,
        )
        .add_global_label("ldap_uri", config.common.ldap_config.uri.clone());
    builder.install()?;

    let program_start_timestamp = Instant::now();

    let tracker = TaskTracker::new();
    let cancel_token_orig = CancellationToken::new();

    let cancel_token = cancel_token_orig.clone();
    tracker.spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to register ctrl-c handler: {}", e);
            tracing::warn!("Program will work. But killing it can be hard");
            return;
        };
        tracing::info!("Received ctrl-c");

        tracing::info!("Shutting down");
        cancel_token.cancel();
    });

    let cancel_token = cancel_token_orig.clone();
    tracker.spawn(async move {
        loop {
            counter!("internal.runtime.seconds_active")
                .absolute(program_start_timestamp.elapsed().as_secs());

            describe_counter!(
                "internal.runtime.seconds_active",
                "How long o11y-389ds-rs daemon has been already running"
            );

            gauge!("internal.scrape_interval_seconds")
                .set(config.exporter.scrape_interval_seconds as f64);
            gauge!(
                "internal.exporter_info",
                "version" => env!("CARGO_PKG_VERSION"),
                "name" => env!("CARGO_PKG_NAME")
            )
            .set(1);

            select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                    config.exporter.scrape_interval_seconds,
                )) => {

                },
                _ = cancel_token.cancelled() => {
                    break
                }
            }
        }
    });

    let cancel_token = cancel_token_orig.clone();
    let config_clone = config.clone();
    if config.exporter.scrape_flags.ldap_monitoring {
        tracker.spawn(async move {
            let mut common_data = MetricsCommonData::default();
            loop {
                let health_gauge = gauge!("internal.health.ldap_monitoring",);
                describe_gauge!(
                    "internal.health.ldap_monitoring",
                    "LDAP cn=monitor scraper status"
                );
                if let Err(error) =
                    get_ldap_metrics(&config_clone.common.ldap_config, &mut common_data).await
                {
                    tracing::error!("Error: {}", error);
                    health_gauge.set(0);
                } else {
                    health_gauge.set(1);
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        config.exporter.scrape_interval_seconds,
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        })
    } else {
        tracker.spawn(async move {
            tracing::info!("LDAP cn=monitoring parsing disabled");
        })
    };

    let cancel_token = cancel_token_orig.clone();
    let config_clone = config.clone();
    if config.exporter.scrape_flags.gids_info {
        tracker.spawn(async move {
            loop {
                let health_gauge = gauge!("internal.health.gids",);
                describe_gauge!("internal.health.gids", "GIDs scraper status");

                if let Err(error) = get_gids_metrics(&config_clone.common.ldap_config).await {
                    tracing::error!("Error: {}", error);
                    health_gauge.set(0);
                } else {
                    health_gauge.set(1);
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        config.exporter.scrape_interval_seconds,
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        })
    } else {
        tracker.spawn(async move {
            tracing::info!("GIDs metric parsing disabled");
        })
    };

    let cancel_token = cancel_token_orig.clone();
    let config_clone = config.clone();
    if config.exporter.scrape_flags.replication_status {
        tracker.spawn(async move {
            let mut common_data = ReplicationCommonData::default();
            let health_gauge = gauge!("internal.health.replication",);
            describe_gauge!("internal.health.replication", "Replica scraper status");

            loop {
                if let Err(error) =
                    get_ldap_replica_metrics(&config_clone.common.ldap_config, &mut common_data)
                        .await
                {
                    tracing::error!("Error: {}", error);
                    health_gauge.set(0);
                } else {
                    health_gauge.set(1);
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        config.exporter.scrape_interval_seconds,
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        })
    } else {
        tracker.spawn(async move {
            tracing::info!("Replica metric parsing disabled");
        })
    };

    let cancel_token = cancel_token_orig.clone();
    let config_clone = config.clone();
    if config.exporter.scrape_flags.dsctl {
        tracker.spawn(async move {
            let mut common_data = DsctlCommonData::default();
            let health_gauge = gauge!("internal.health.dsctl",);
            describe_gauge!("internal.health.dsctl", "cli scraper status");
            loop {
                if let Err(error) =
                    get_dsctl_metrics(&config_clone.common.scrapers.dsctl, &mut common_data).await
                {
                    tracing::error!("Error: {}", error);
                    health_gauge.set(0);
                } else {
                    health_gauge.set(1);
                }

                select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(
                        config.exporter.scrape_interval_seconds,
                    )) => {

                    },
                    _ = cancel_token.cancelled() => {
                        break
                    }
                }
            }
        })
    } else {
        tracker.spawn(async move {
            tracing::info!("dsctl metric parsing disabled");
        })
    };

    setup_query_checks(cancel_token_orig.clone(), config.clone(), &tracker).await?;

    tracker.close();
    tracker.wait().await;

    Ok(())
}

async fn handle_query(query: CustomQuery) -> Result<()> {
    let metrics = query.get_metrics().await?;

    let labels = vec![("query", query.name)];

    let g = gauge!("custom_query.duration_ms", &labels);
    g.set(metrics.query_time.as_millis() as f64);

    let g = gauge!("custom_query.object_count", &labels);
    g.set(metrics.object_count as f64);

    let g = gauge!("custom_query.attrs_count", &labels);
    g.set(metrics.attrs_count as f64);

    let g = gauge!("custom_query.ldap_code", &labels);
    g.set(metrics.ldap_code as f64);

    Ok(())
}
