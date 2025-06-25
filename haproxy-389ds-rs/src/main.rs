pub mod haproxy;
pub mod ldap_health;
pub mod web;

use clap::{ArgGroup, Parser};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

fn default_scrape_interval_seconds() -> u64 {
    5
}

fn default_expose_port() -> u16 {
    6699
}

fn default_expose_address() -> String {
    "0.0.0.0".to_string()
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

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub haproxy: HaproxyConfig,

    #[serde(flatten)]
    pub common: internal::config::CommonConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HaproxyConfig {
    #[serde(default = "default_expose_port")]
    pub expose_port: u16,

    #[serde(default = "default_expose_address")]
    pub expose_address: String,

    #[serde(default = "default_scrape_interval_seconds")]
    pub scrape_interval_seconds: u64,

    #[serde(default)]
    pub scrape_flags: ScrapeFlags,
}

impl Default for HaproxyConfig {
    fn default() -> Self {
        Self {
            expose_port: default_expose_port(),
            expose_address: default_expose_address(),
            scrape_interval_seconds: default_scrape_interval_seconds(),
            scrape_flags: ScrapeFlags::default(),
        }
    }
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

#[tokio::main]
async fn main() {
    let _args = Args::parse();
}
