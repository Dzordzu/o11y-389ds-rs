use std::collections::HashMap;

use anyhow::{anyhow, Result};
use clap::{ArgGroup, Args, Parser, Subcommand};
use internal::{Bind, LdapConfig};

#[derive(Copy, Clone, Debug, Default)]
pub enum ReturnCode {
    #[default]
    Ok = 0,
    Warning = 1,
    Critical = 2,
    Unknown = 3,
}

impl ReturnCode {
    pub fn warn(&mut self) {
        if let ReturnCode::Ok = self {
            *self = ReturnCode::Warning
        }
    }

    pub fn crit(&mut self) {
        match self {
            ReturnCode::Ok | ReturnCode::Warning => *self = ReturnCode::Critical,
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Copy, Default)]
pub enum PerfDataValue {
    Int(u64),
    Float(f64),

    /// Please, don't use it directly. Same as None
    #[default]
    Empty,
}

#[allow(non_snake_case)]
fn PDV<T: Into<PerfDataValue>>(v: T) -> Option<PerfDataValue> {
    Some(v.into())
}

impl From<u64> for PerfDataValue {
    fn from(value: u64) -> Self {
        PerfDataValue::Int(value)
    }
}

impl From<u32> for PerfDataValue {
    fn from(value: u32) -> Self {
        PerfDataValue::Int(value as u64)
    }
}

impl From<f64> for PerfDataValue {
    fn from(value: f64) -> Self {
        PerfDataValue::Float(value)
    }
}

impl std::fmt::Display for PerfDataValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            PerfDataValue::Int(x) => x.to_string(),
            PerfDataValue::Float(x) => x.to_string(),
            PerfDataValue::Empty => "".to_string(),
        };
        f.write_str(&value.to_string())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PerfData {
    pub val: Option<PerfDataValue>,
    pub min: Option<PerfDataValue>,
    pub max: Option<PerfDataValue>,
    pub warn: Option<PerfDataValue>,
    pub crit: Option<PerfDataValue>,
    pub unit: Option<String>,
}

impl PerfData {
    pub fn to_nagios_str(&self) -> String {
        format!(
            "{val}{unit};{warn};{crit};{min};{max} ",
            val = self.val.unwrap_or_default(),
            unit = self.unit.clone().unwrap_or_default(),
            warn = self.warn.unwrap_or_default(),
            crit = self.crit.unwrap_or_default(),
            min = self.min.unwrap_or_default(),
            max = self.max.unwrap_or_default()
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Nagios {
    pub return_code: ReturnCode,
    pub description: Option<String>,
    pub perfdata: HashMap<String, PerfData>,
}

impl Nagios {
    pub fn exit_with_message(&self) {
        let desc = match self.return_code {
            ReturnCode::Ok => "OK",
            ReturnCode::Warning => "WARN",
            ReturnCode::Critical => "CRIT",
            ReturnCode::Unknown => "UNKNOWN",
        }
        .to_string();

        let perf_data = self.perfdata.iter().fold(String::new(), |acc, (k, v)| {
            let k = k.replace("'", "").replace("=", "");
            format!("{acc}'{k}'={}", v.to_nagios_str())
        });

        let desc = format!(
            "{}: {} | {}",
            desc,
            self.description.as_ref().unwrap_or(&String::new()),
            perf_data
        );

        println!("{desc}");
        std::process::exit(self.return_code as i32);
    }
}

#[derive(Args, Clone, Debug)]
pub struct Diskspace {
    #[arg(short, long)]
    pub warn_percent_used: Option<f64>,

    #[arg(short, long)]
    pub crit_percent_used: Option<f64>,

    #[arg(short = 'W', long)]
    pub warn_absolute_available: Option<u64>,

    #[arg(short = 'C', long)]
    pub crit_absolute_available: Option<u64>,

    #[arg(short, long)]
    pub partitions: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub struct RecentRestart {
    #[arg(short, long)]
    pub warn_if_less_than: Option<u64>,
}

#[derive(Args, Clone, Debug)]
pub struct Errors {
    #[arg(short = 'W', long)]
    pub warn_sum: Option<u64>,

    #[arg(short = 'C', long)]
    pub crit_sum: Option<u64>,

    #[arg(short, long)]
    pub warn: Option<u64>,

    #[arg(short, long)]
    pub crit: Option<u64>,

    /// Names of the error keys to include
    #[arg(short, long)]
    pub names: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub struct Connections {
    #[arg(short, long)]
    pub warn: Option<u64>,

    #[arg(short, long)]
    pub crit: Option<u64>,

    /// DNs of the connections to include
    #[arg(short, long)]
    pub dn: Vec<String>,

    /// IP addresses of the connections to include
    #[arg(short, long)]
    pub ip: Vec<String>,

    /// Skip connections with these DNs
    #[arg(short = 'D', long)]
    pub exclude_dn: Vec<String>,

    /// Skip connections with these IP addresses
    #[arg(short = 'I', long)]
    pub exclude_ip: Vec<String>,

    /// By default check include integrity validation between snmp, monitor and counted connections
    /// numbers. This can be skipped by setting this flag
    #[arg(short, long, default_value_t = false)]
    pub skip_integrity: bool,

    /// Debug option showing more information. Will not work well for checks, but useful for
    /// debugging them
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Args, Clone, Debug)]
pub struct MissingGids {
    #[arg(short, long)]
    pub warn_groups: Option<u64>,

    #[arg(short, long)]
    pub crit_groups: Option<u64>,

    #[arg(short = 'W', long)]
    pub warn_users: Option<u64>,

    #[arg(short = 'C', long)]
    pub crit_users: Option<u64>,
}

#[derive(Args, Clone, Debug)]
#[clap(group = ArgGroup::new("req").required(true).multiple(false))]
pub struct CheckIntMetric {
    /// Debug option. Will not work well for checks, but useful for debugging
    #[clap(group = "req")]
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// By default checks are using "less than". Set this to true to use "greater than"
    #[arg(short = 'r', long, default_value_t = false)]
    pub revert_comparsion: bool,

    /// Source of the scraped metric
    #[clap(group = "req")]
    #[arg(short = 's', long, requires = "metric")]
    pub metric_source: Option<String>,

    /// Name of the metric
    #[arg(short = 'm', long)]
    pub metric: Option<String>,

    #[arg(short, long)]
    pub warn: Option<u64>,

    #[arg(short, long)]
    pub crit: Option<u64>,
}

#[derive(Args, Clone, Debug)]
pub struct AgreementDuration {
    #[arg(short, long)]
    pub warn: Option<u64>,

    #[arg(short, long)]
    pub crit: Option<u64>,
}

#[derive(Args, Clone, Debug)]
pub struct AgreementSkipped {
    #[arg(short, long)]
    pub warn: Option<u64>,

    #[arg(short, long)]
    pub crit: Option<u64>,
}

#[derive(Args, Clone, Debug)]
pub struct AgreementStatus {
    /// By default RUV is also checked. Set this to true to skip this check
    #[arg(short = 'R', long, default_value_t = false)]
    pub no_ruv: bool,
}

#[derive(Args, Clone, Debug)]
#[command(disable_help_flag = true)]
pub struct CliHealthcheck {
    #[arg(short = 'T', long)]
    pub timeout: Option<u64>,

    #[arg(short, long, default_value=internal::cli::DEFAULT_INSTANCE)]
    pub instance: String,

    /// Overall errors warn
    #[arg(short, long)]
    pub warn: Option<u64>,

    /// Overall errors crit
    #[arg(short, long)]
    pub crit: Option<u64>,

    /// LOW severity errors warn
    #[arg(short = 'l', long)]
    pub warn_low: Option<u64>,

    /// LOW severity errors crit
    #[arg(short = 'L', long)]
    pub crit_low: Option<u64>,

    /// MEDIUM severity errors warn
    #[arg(short = 'm', long)]
    pub warn_medium: Option<u64>,

    /// MEDIUM severity errors crit
    #[arg(short = 'M', long)]
    pub crit_medium: Option<u64>,

    /// HIGH severity errors warn
    #[arg(short = 'h', long)]
    pub warn_high: Option<u64>,

    /// HIGH severity errors crit
    #[arg(short = 'H', long)]
    pub crit_high: Option<u64>,

    #[clap(short='?', long, action = clap::ArgAction::Help, help = "Print help information")]
    help: Option<bool>,
}

#[derive(Args, Clone, Debug)]
pub struct CustomQueryTime {
    #[arg(short = 'f', long, required = true)]
    pub filter: String,

    #[arg(short = 'w', long)]
    pub warn: Option<u64>,

    #[arg(short = 'c', long)]
    pub crit: Option<u64>,
}

#[derive(Args, Clone, Debug)]
pub struct CustomQueryIntegrity {
    /// An additional host to check against
    #[arg(short = 'H', long = "host", required = true)]
    pub host: String,

    #[arg(short = 'f', long, required = true)]
    pub filter: String,

    /// Attributes to get
    #[arg(short = 'a', long)]
    pub attributes: Vec<String>,

    /// Check integrity using sha256
    #[arg(short = 'S', long, default_value_t = false)]
    pub sha256_integrity: bool,

    /// Check integrity using number of ldap entries
    #[arg(short = 'E', long, default_value_t = false)]
    pub entries_count_integrity: bool,

    /// Check integrity using number of ldap attributes
    #[arg(short = 'A', long, default_value_t = false)]
    pub attributes_count_integrity: bool,

    /// Check integrity using number of ldap bytes in the returned attributes values
    #[arg(short = 'B', long, default_value_t = false)]
    pub bytes_size_integrity: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum CheckVariant {
    /// Check any scraped metric (integers). Fallback for missing options
    CheckIntMetric(CheckIntMetric),
    /// Check status of the replication
    AgreementStatus(AgreementStatus),
    /// Check skipped entries in the replication
    AgreementSkipped(AgreementSkipped),
    /// Check duration of the replication
    AgreementDuration(AgreementDuration),
    /// Check if there are primary gids that are not present as posixGroup
    MissingGids(MissingGids),
    /// Check number of active connections
    Connections(Connections),
    /// Check cumber of errors: Errors + SecurityErrors + BindSecurityErrors
    Errors(Errors),
    /// Check if daemon has been recently restarted
    RecentRestart(RecentRestart),
    /// Check if disk space is low (declared by the daemon)
    Diskspace(Diskspace),
    /// Check health using dsctl cli
    CliHealthcheck(CliHealthcheck),
    /// Check custom query times
    CustomQueryTime(CustomQueryTime),
    /// Check custom query integrity
    CustomQueryIntegrity(CustomQueryIntegrity),
}

/// Perform nagios checks on the 389ds. All limits are using >= or <= comparsions, unless stated otherwise
#[derive(Parser, Clone, Debug)]
#[clap(group(ArgGroup::new("bind").requires_all(["binddn", "bindpass"]).multiple(true)))]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: CheckVariant,

    #[clap(short = 'c', long)]
    config: Option<std::path::PathBuf>,

    /// Disable TLS cert verification
    #[clap(short = 'C', long, default_value_t = false)]
    skip_cert_verification: bool,

    #[clap(short = 'H', long)]
    host: Option<String>,

    #[clap(short = 'D', long)]
    #[clap(group = "bind")]
    binddn: Option<String>,

    #[clap(short = 'w', long)]
    #[clap(group = "bind")]
    bindpass: Option<String>,

    #[clap(short = 'b', long)]
    basedn: Option<String>,

    #[clap(short = 'P', long)]
    page_size: Option<i32>,
}

pub async fn command_select(config: LdapConfig, args: Cli, result: &mut Nagios) -> Result<()> {
    let mut ldap = config.connect().await?;

    match &args.subcommand {
        CheckVariant::CheckIntMetric(config) => {
            let monitor = internal::monitor::LdapMonitor::scrape(&mut ldap)
                .await?
                .int_metrics;
            let snmp = internal::monitor::LdapSNMP::scrape(&mut ldap)
                .await?
                .int_metrics;

            let map: HashMap<String, HashMap<String, i64>> = HashMap::from([
                (
                    "monitor".to_string(),
                    monitor.into_iter().map(|(k, v)| (k, v as i64)).collect(),
                ),
                (
                    "snmp".to_string(),
                    snmp.into_iter().map(|(k, v)| (k, v as i64)).collect(),
                ),
            ]);

            if config.debug {
                println!("{map:#?}");
            } else {
                let metric_source = config
                    .metric_source
                    .clone()
                    .ok_or(anyhow!("Missing metric source"))?;
                let metric = config
                    .metric
                    .clone()
                    .ok_or(anyhow!("Missing metric source"))?;

                let metric_val = map
                    .get(&metric_source)
                    .ok_or(anyhow!("No such a metric source"))?
                    .get(&metric)
                    .ok_or(anyhow!("No such a metric"))?;

                let unit = match &metric {
                    x if x.contains("bytes") => Some("B".to_string()),
                    _ => None,
                };

                result.description = Some(format!("{}_{}", &metric_source, &metric));
                result.perfdata = HashMap::from([(
                    String::from("value"),
                    PerfData {
                        val: PDV(*metric_val as f64),
                        warn: config.warn.map(PDV).unwrap_or_default(),
                        crit: config.crit.map(PDV).unwrap_or_default(),
                        unit,
                        ..Default::default()
                    },
                )]);

                if let Some(warn) = config.warn {
                    if (config.revert_comparsion && *metric_val <= warn as i64)
                        || (!config.revert_comparsion && *metric_val >= warn as i64)
                    {
                        result.return_code.warn();
                    }
                }

                if let Some(crit) = config.crit {
                    if (config.revert_comparsion && *metric_val <= crit as i64)
                        || (!config.revert_comparsion && *metric_val >= crit as i64)
                    {
                        result.return_code.crit();
                    }
                }
            }
        }
        CheckVariant::AgreementStatus(config) => {
            result.description = Some("agreement status".to_string());

            for agreement in internal::replica::Agreement::scrape(&mut ldap).await? {
                let status = agreement.status;

                if status.ldap_rc != 0 {
                    result.return_code.crit();
                }

                if status.state != "green" {
                    result.return_code.crit();
                }

                if status.repl_rc != 0 {
                    result.return_code.crit();
                }

                result.perfdata.insert(
                    agreement.cn.clone(),
                    PerfData {
                        val: PDV(0_u64),
                        crit: PDV(1_u64),
                        min: PDV(0_u64),
                        ..Default::default()
                    },
                );

                if !config.no_ruv {
                    use internal::replica::Ruv;

                    for ruv in agreement.ruvs {
                        match ruv {
                            Ruv::ReplicaGen(_) => {}
                            Ruv::BrokenReplication { replica_id, server } => {
                                result.perfdata.insert(
                                    format!(
                                        "{} RUV server({}) replica({})",
                                        &agreement.cn, server, replica_id
                                    ),
                                    PerfData {
                                        val: PDV(1_u64),
                                        crit: PDV(1_u64),
                                        min: PDV(0_u64),
                                        ..Default::default()
                                    },
                                );

                                result.return_code.crit();
                            }
                            Ruv::Info {
                                replica_id, server, ..
                            } => {
                                result.perfdata.insert(
                                    format!(
                                        "{} RUV server({}) replica({})",
                                        &agreement.cn, server, replica_id
                                    ),
                                    PerfData {
                                        val: PDV(0_u64),
                                        crit: PDV(1_u64),
                                        min: PDV(0_u64),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
        CheckVariant::AgreementSkipped(config) => {
            result.description = Some("agreement objects skipped".to_string());
            for agreement in internal::replica::Agreement::scrape(&mut ldap).await? {
                for changes_sent in agreement.changes_sent {
                    result.perfdata.insert(
                        format!("{} replica_{}", agreement.cn, changes_sent.replica_id),
                        PerfData {
                            val: PDV(changes_sent.changes_skipped),
                            warn: config.warn.map(PDV).unwrap_or_default(),
                            crit: config.crit.map(PDV).unwrap_or_default(),
                            unit: None,
                            min: PDV(0_u64),
                            ..Default::default()
                        },
                    );

                    if let Some(warn) = config.warn {
                        if changes_sent.changes_skipped >= warn {
                            result.return_code.warn();
                        }
                    }

                    if let Some(crit) = config.crit {
                        if changes_sent.changes_skipped >= crit {
                            result.return_code.crit();
                        }
                    }
                }
            }
        }
        CheckVariant::AgreementDuration(config) => {
            result.description = Some("agreements duration (seconds)".to_string());
            for agreement in internal::replica::Agreement::scrape(&mut ldap).await? {
                result.perfdata.insert(
                    agreement.cn,
                    PerfData {
                        val: PDV(agreement.last_update_duration_seconds as u64),
                        warn: config.warn.map(PDV).unwrap_or_default(),
                        crit: config.crit.map(PDV).unwrap_or_default(),
                        unit: Some("s".to_string()),
                        min: PDV(0_u64),
                        ..Default::default()
                    },
                );

                if let Some(warn) = config.warn {
                    if agreement.last_update_duration_seconds as u64 >= warn {
                        result.return_code.warn();
                    }
                }

                if let Some(crit) = config.crit {
                    if agreement.last_update_duration_seconds as u64 >= crit {
                        result.return_code.crit();
                    }
                }
            }
        }
        CheckVariant::MissingGids(mg_config) => {
            let gids = internal::gids::missing_gids_to_uid_mapping(&config).await?;
            let config = mg_config;

            result.description = Some("Missing gids".to_string());

            result.perfdata.insert(
                "total_gids".to_string(),
                PerfData {
                    val: PDV(gids.len() as u64),
                    warn: config.warn_groups.map(PDV).unwrap_or_default(),
                    crit: config.crit_groups.map(PDV).unwrap_or_default(),
                    ..Default::default()
                },
            );

            result.perfdata.extend(gids.iter().map(|(gid, uids)| {
                (
                    format!("gid[{gid}]"),
                    PerfData {
                        val: PDV(*uids),
                        ..Default::default()
                    },
                )
            }));

            let total = gids.len() as u64;
            if let Some(warn) = config.warn_groups {
                if total >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }
            if let Some(crit) = config.crit_groups {
                if total >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }

            let total_users = gids.values().sum::<u64>();
            result.perfdata.insert(
                "total_users".to_string(),
                PerfData {
                    val: PDV(total_users),
                    warn: config.warn_users.map(PDV).unwrap_or_default(),
                    crit: config.crit_users.map(PDV).unwrap_or_default(),
                    ..Default::default()
                },
            );

            if let Some(warn) = config.warn_users {
                if total_users >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(crit) = config.crit_users {
                if total_users >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }
        }
        CheckVariant::Connections(config) => {
            let (connections, monitor_connections) = {
                let base = internal::monitor::LdapMonitor::scrape(&mut ldap).await?;
                (
                    base.connections,
                    base.int_metrics
                        .get("currentconnections")
                        .copied()
                        .unwrap_or(0_u64),
                )
            };

            if !config.skip_integrity {
                let snmp_connections = internal::monitor::LdapSNMP::scrape(&mut ldap)
                    .await?
                    .int_metrics
                    .get("connections")
                    .copied()
                    .unwrap_or(0_i32) as u64;

                let counted = connections.vec().len();

                if config.debug {
                    println!("Connections (cn=monitor): {}", monitor_connections);
                    println!("Connections (cn=snmp,cn=monitor): {}", snmp_connections);
                    println!("Counted: {}", counted);
                }

                if counted as u64 != monitor_connections || monitor_connections != snmp_connections
                {
                    result.perfdata = HashMap::from([
                        (
                            "reported_connections".to_string(),
                            PerfData {
                                val: PDV(monitor_connections),
                                ..Default::default()
                            },
                        ),
                        (
                            "reported_connections_snmp".to_string(),
                            PerfData {
                                val: PDV(snmp_connections),
                                ..Default::default()
                            },
                        ),
                        (
                            "counted".to_string(),
                            PerfData {
                                val: PDV(counted as u64),
                                ..Default::default()
                            },
                        ),
                    ]);
                    return Err(anyhow!(
                        "Inconsistent number of connections between reported values"
                    ));
                }
            }

            let config_dn_lowercase: Vec<String> =
                config.dn.iter().map(|x| x.to_lowercase()).collect();
            let config_exclude_dn_lowercase: Vec<String> =
                config.exclude_dn.iter().map(|x| x.to_lowercase()).collect();

            if config.debug {
                println!("------------------------------");
                println!("Connections before filtering: ");
                for c in connections.vec() {
                    println!("{:?}", c);
                }
            }

            let connections: Vec<internal::monitor::LdapConnection> = connections
                .consume_vec()
                .into_iter()
                .filter(|x| {
                    (config_dn_lowercase.is_empty()
                        || config_dn_lowercase.contains(&x.dn.to_lowercase()))
                        && (config.ip.is_empty() || config.ip.contains(&x.ip))
                        && !(config_exclude_dn_lowercase.contains(&x.dn.to_lowercase()))
                        && !(config.exclude_ip.contains(&x.ip))
                })
                .collect();

            if config.debug {
                println!("------------------------------");
                println!("Connections after filtering: ");
                for c in &connections {
                    println!("{:?}", c);
                }
            }

            result.description = Some("389ds reported connections".to_string());

            result.perfdata = HashMap::from([(
                "connections".to_string(),
                PerfData {
                    min: PDV(0_u64),
                    val: PDV(connections.len() as u64),
                    warn: config.warn.map(PDV).unwrap_or_default(),
                    crit: config.crit.map(PDV).unwrap_or_default(),
                    ..Default::default()
                },
            )]);

            if let Some(warn) = config.warn {
                if connections.len() as u64 >= warn {
                    result.return_code.warn()
                }
            }

            if let Some(crit) = config.crit {
                if connections.len() as u64 >= crit {
                    result.return_code.crit()
                }
            }
        }
        CheckVariant::Errors(config) => {
            let snmp = internal::monitor::LdapSNMP::scrape(&mut ldap).await?;
            let error_keys = snmp.int_metrics.keys().filter(|x| {
                x.contains("error") && (config.names.is_empty() || config.names.contains(x))
            });

            let errors: HashMap<String, u64> = error_keys
                .map(|key| {
                    (
                        key.to_string(),
                        *snmp
                            .int_metrics
                            .get(key)
                            .expect("These keys should be checked before!")
                            as u64,
                    )
                })
                .collect();

            let errors_sum: u64 = errors.values().sum();

            result.description = Some("389ds errors in the SNMP monitor".to_string());

            result.perfdata = HashMap::from([(
                "errors_sum".to_string(),
                PerfData {
                    min: PDV(0_u64),
                    val: PDV(errors_sum),
                    warn: config.warn_sum.map(PDV).unwrap_or_default(),
                    crit: config.crit_sum.map(PDV).unwrap_or_default(),
                    ..Default::default()
                },
            )]);

            result.perfdata.extend(errors.iter().map(|(key, value)| {
                (
                    key.to_string(),
                    PerfData {
                        min: PDV(0_u64),
                        val: PDV(*value),
                        warn: config.warn.map(PDV).unwrap_or_default(),
                        crit: config.crit.map(PDV).unwrap_or_default(),
                        ..Default::default()
                    },
                )
            }));

            if let Some(warn_sum) = config.warn_sum {
                if errors_sum >= warn_sum {
                    result.return_code.warn()
                }
            }

            if let Some(crit_sum) = config.crit_sum {
                if errors_sum >= crit_sum {
                    result.return_code.crit()
                }
            }

            errors.iter().for_each(|(_, value)| {
                if let Some(warn) = config.warn {
                    if *value >= warn {
                        result.return_code.warn()
                    }
                }
                if let Some(crit) = config.crit {
                    if *value >= crit {
                        result.return_code.crit()
                    }
                }
            });
        }
        CheckVariant::RecentRestart(config) => {
            const STARTTIME: &str = "starttime";
            const CURRTIME: &str = "currenttime";

            let metrics = internal::monitor::LdapMonitor::scrape(&mut ldap).await?;

            let starttime = metrics
                .date_metrics
                .get(STARTTIME)
                .ok_or(anyhow!("Missing starttime"))?;
            let currenttime = metrics
                .date_metrics
                .get(CURRTIME)
                .ok_or(anyhow!("Missing starttime"))?;

            let difference_seconds =
                currenttime.and_utc().timestamp() - starttime.and_utc().timestamp();

            result.perfdata = HashMap::from([(
                "seconds_since_last_restart".to_string(),
                PerfData {
                    min: PDV(0.0),
                    warn: config.warn_if_less_than.and_then(PDV),
                    val: PDV(difference_seconds as f64),
                    unit: Some("s".to_string()),
                    ..Default::default()
                },
            )]);

            result.description = Some("Seconds since last restart of the 390ds".to_string());

            if let Some(limit) = config.warn_if_less_than {
                if (difference_seconds as u64) <= limit {
                    result.return_code.warn()
                }
            }
        }
        CheckVariant::Diskspace(config) => {
            const USE_PERCENTAGE: &str = "use%";
            const AVAILABLE: &str = "available";

            let partitions: Vec<_> = internal::monitor::LdapDisk::scrape(&mut ldap)
                .await?
                .partitions
                .into_iter()
                .filter(|x| config.partitions.is_empty() || config.partitions.contains(&x.0))
                .collect();

            result.description = Some(String::from("disk free space (389ds reported)"));

            for partition in &partitions {
                result.perfdata.extend([
                    (
                        format!("use_percentage {}", partition.0),
                        PerfData {
                            min: PDV(1.0),
                            max: PDV(100.0),
                            warn: config.warn_percent_used.and_then(PDV),
                            crit: config.crit_percent_used.and_then(PDV),
                            unit: Some("%".to_string()),
                            val: partition
                                .1
                                .int_metrics
                                .get(USE_PERCENTAGE)
                                .copied()
                                .and_then(PDV),
                        },
                    ),
                    (
                        format!("available_space {}", partition.0),
                        PerfData {
                            min: PDV(0_u64),
                            max: None,
                            warn: config.warn_absolute_available.and_then(PDV),
                            crit: config.crit_absolute_available.and_then(PDV),
                            unit: Some("B".to_string()),
                            val: partition
                                .1
                                .int_metrics
                                .get(AVAILABLE)
                                .copied()
                                .and_then(PDV),
                        },
                    ),
                ]);

                let use_percentage = partition
                    .1
                    .int_metrics
                    .get(USE_PERCENTAGE)
                    .copied()
                    .unwrap_or(100) as f64;

                let available_absolute =
                    partition.1.int_metrics.get(AVAILABLE).copied().unwrap_or(0);

                if let Some(limit) = config.warn_percent_used {
                    if use_percentage >= limit {
                        result.return_code.warn();
                    }
                }

                if let Some(limit) = config.warn_absolute_available {
                    if available_absolute <= limit {
                        result.return_code.warn();
                    }
                }

                if let Some(limit) = config.crit_percent_used {
                    if use_percentage >= limit {
                        result.return_code.crit();
                    }
                }

                if let Some(limit) = config.crit_absolute_available {
                    if available_absolute <= limit {
                        result.return_code.crit();
                    }
                }
            }
        }
        CheckVariant::CliHealthcheck(config) => {
            let cli_conf = internal::cli::CommandConfig {
                timeout_seconds: config.timeout,
                instance_name: config.instance.clone(),
            };

            let healthchecks = cli_conf.healthcheck().await?;

            let low_severity = healthchecks
                .iter()
                .filter(|x| x.severity == internal::cli::Severity::LOW)
                .count() as u64;

            let high_severity = healthchecks
                .iter()
                .filter(|x| x.severity == internal::cli::Severity::HIGH)
                .count() as u64;

            let medium_severity = healthchecks
                .iter()
                .filter(|x| x.severity == internal::cli::Severity::MEDIUM)
                .count() as u64;

            let all_severity = low_severity + high_severity + medium_severity;

            result.description = Some(String::from("CLI healthcheck"));
            result.perfdata.extend([
                (
                    "all_severity".to_string(),
                    PerfData {
                        min: PDV(0_u64),
                        val: PDV(all_severity),
                        warn: config.warn.map(PDV).unwrap_or_default(),
                        crit: config.crit.map(PDV).unwrap_or_default(),
                        ..Default::default()
                    },
                ),
                (
                    "low_severity".to_string(),
                    PerfData {
                        min: PDV(0_u64),
                        val: PDV(low_severity),
                        warn: config.warn_low.map(PDV).unwrap_or_default(),
                        crit: config.crit_low.map(PDV).unwrap_or_default(),
                        ..Default::default()
                    },
                ),
                (
                    "medium_severity".to_string(),
                    PerfData {
                        min: PDV(0_u64),
                        val: PDV(medium_severity),
                        warn: config.warn_medium.map(PDV).unwrap_or_default(),
                        crit: config.crit_medium.map(PDV).unwrap_or_default(),
                        ..Default::default()
                    },
                ),
                (
                    "high_severity".to_string(),
                    PerfData {
                        min: PDV(0_u64),
                        val: PDV(high_severity),
                        warn: config.warn_high.map(PDV).unwrap_or_default(),
                        crit: config.crit_high.map(PDV).unwrap_or_default(),
                        ..Default::default()
                    },
                ),
            ]);

            if let Some(warn) = config.warn {
                if all_severity >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(warn) = config.warn_low {
                if low_severity >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(warn) = config.warn_medium {
                if medium_severity >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(warn) = config.warn_high {
                if high_severity >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(crit) = config.crit {
                if all_severity >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }

            if let Some(crit) = config.crit_low {
                if low_severity >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }

            if let Some(crit) = config.crit_medium {
                if medium_severity >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }

            if let Some(crit) = config.crit_high {
                if high_severity >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }
        }
        CheckVariant::CustomQueryTime(cqt_config) => {
            let cq = internal::query::CustomQuery::new(
                "query".to_string(),
                cqt_config.filter.clone(),
                config,
            );

            let metrics = cq.get_metrics().await?;

            result.description = Some("query time".to_string());
            result.perfdata.extend([(
                "query_time".to_string(),
                PerfData {
                    min: PDV(0_u64),
                    val: PDV(metrics.query_time.as_millis() as u64),
                    warn: cqt_config.warn.map(PDV).unwrap_or_default(),
                    crit: cqt_config.crit.map(PDV).unwrap_or_default(),
                    ..Default::default()
                },
            )]);

            if let Some(warn) = cqt_config.warn {
                if metrics.query_time.as_millis() as u64 >= warn {
                    result.return_code = ReturnCode::Warning;
                }
            }

            if let Some(crit) = cqt_config.crit {
                if metrics.query_time.as_millis() as u64 >= crit {
                    result.return_code = ReturnCode::Critical;
                }
            }
        }
        CheckVariant::CustomQueryIntegrity(cqi_config) => {
            let (object_number, bytes_size, attr_number, checksum) = {
                let mut custom_query = internal::query::CustomQuery::new(
                    "query".to_string(),
                    cqi_config.filter.clone(),
                    config.clone(),
                );
                custom_query.attrs = cqi_config.attributes.clone();
                let metrics = custom_query.get_metrics().await?;

                (
                    metrics.object_count,
                    metrics.bytes,
                    metrics.attrs_count,
                    metrics.sha256_checksum,
                )
            };

            struct Integrity {
                on: bool,
                bs: bool,
                an: bool,
                cs: bool,

                on_num: u64,
                bs_num: u64,
                an_num: u64,
                cs_val: String,

                on_num_compared: u64,
                bs_num_compared: u64,
                an_num_compared: u64,
                cs_val_reported: String,
            }

            impl Integrity {
                fn new(on_num: u64, bs_num: u64, an_num: u64, cs_val: String) -> Integrity {
                    Integrity {
                        on: true,
                        bs: true,
                        an: true,
                        cs: true,
                        on_num,
                        bs_num,
                        an_num,
                        cs_val: cs_val.to_string(),
                        on_num_compared: on_num,
                        bs_num_compared: bs_num,
                        an_num_compared: an_num,
                        cs_val_reported: cs_val.to_string(),
                    }
                }
                fn compare(&mut self, on_num: u64, bs_num: u64, an_num: u64, cs_val: String) {
                    if self.on_num != on_num {
                        self.on = false;
                        self.on_num_compared = on_num;
                    }
                    if self.bs_num != bs_num {
                        self.bs = false;
                        self.bs_num_compared = bs_num;
                    }
                    if self.an_num != an_num {
                        self.an = false;
                        self.an_num_compared = an_num;
                    }
                    if self.cs_val != cs_val {
                        self.cs = false;
                        self.cs_val_reported = cs_val;
                    }
                }
            }

            let mut integrity =
                Integrity::new(object_number, bytes_size, attr_number, checksum.clone());

            let mut config = config.clone();
            config.uri = cqi_config.host.clone();

            let mut custom_query = internal::query::CustomQuery::new(
                "query".to_string(),
                cqi_config.filter.clone(),
                config.clone(),
            );
            custom_query.attrs = cqi_config.attributes.clone();
            let metrics = custom_query.get_metrics().await?;

            integrity.compare(
                metrics.object_count,
                metrics.bytes,
                metrics.attrs_count,
                metrics.sha256_checksum,
            );

            if !integrity.cs && cqi_config.sha256_integrity {
                result.return_code.crit();
            }

            if !integrity.on && cqi_config.entries_count_integrity {
                result.return_code.crit();
            }

            if !integrity.bs && cqi_config.bytes_size_integrity {
                result.return_code.crit();
            }

            if !integrity.an && cqi_config.attributes_count_integrity {
                result.return_code.crit();
            }

            result.description = Some("query integrity across hosts".to_string());
            result.perfdata.extend([
                (
                    "object_number".to_string(),
                    PerfData {
                        val: PDV(integrity.on_num),
                        ..Default::default()
                    },
                ),
                (
                    "bytes_size".to_string(),
                    PerfData {
                        val: PDV(integrity.bs_num),
                        ..Default::default()
                    },
                ),
                (
                    "attr_number".to_string(),
                    PerfData {
                        val: PDV(integrity.an_num),
                        ..Default::default()
                    },
                ),
                (
                    "object_number_compared".to_string(),
                    PerfData {
                        val: PDV(integrity.on_num_compared),
                        ..Default::default()
                    },
                ),
                (
                    "bytes_size_compared".to_string(),
                    PerfData {
                        val: PDV(integrity.bs_num_compared),
                        ..Default::default()
                    },
                ),
                (
                    "attr_number_compared".to_string(),
                    PerfData {
                        val: PDV(integrity.an_num_compared),
                        ..Default::default()
                    },
                ),
                (
                    "checksum_ok".to_string(),
                    PerfData {
                        val: PDV(integrity.cs as u64),
                        ..Default::default()
                    },
                ),
            ])
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let mut config = if let Some(config) = &args.config {
        let config = String::from_utf8(std::fs::read(config)?)?;
        toml::from_str(&config).unwrap()
    } else {
        LdapConfig::default()
    };

    if let Some(basedn) = &args.basedn {
        config.default_base = basedn.clone();
    }

    if let Some(page_size) = args.page_size {
        config.page_size = page_size;
    }

    if let Some(uri) = &args.host {
        config.uri = uri.clone();
    }

    if args.skip_cert_verification {
        config.verify_certs = false;
    }
    if let Some(bind) = if let Some(binddn) = &args.binddn {
        let bindpass = args.bindpass.clone().unwrap();

        Some(Bind {
            dn: binddn.clone(),
            pass: bindpass,
        })
    } else {
        None
    } {
        config.bind = Some(bind);
    }

    let mut result = Nagios::default();

    let response = command_select(config, args, &mut result).await;

    if let Err(error) = response {
        result.return_code = ReturnCode::Unknown;
        result.description = Some(error.to_string());
    }

    result.exit_with_message();

    Ok(())
}
