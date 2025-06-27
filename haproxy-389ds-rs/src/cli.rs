use clap::{ArgGroup, Parser};
use std::path::PathBuf;

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ArgFlag {
    /// Parse replication entries
    Replication,

    /// Parse monitoring entry
    LdapMonitor,
}

#[derive(Parser)]
#[clap(group(ArgGroup::new("bind").requires_all(["binddn", "bindpass"]).multiple(true)))]
pub struct Args {
    /// Path to the TOML configuration file
    #[clap(short, long)]
    pub config: Option<PathBuf>,

    /// LDAP paging setting
    #[clap(short = 'P', long)]
    pub page_size: Option<i32>,

    /// Disable TLS cert verification
    #[clap(short = 'C', long, default_value_t = false)]
    pub skip_cert_verification: bool,

    #[clap(short = 'a', long)]
    pub expose_address: Option<String>,

    #[clap(short = 'p', long)]
    pub expose_port: Option<u16>,

    #[clap(short = 'b', long)]
    pub basedn: Option<String>,

    #[clap(short = 'D', long)]
    #[clap(group = "bind")]
    pub binddn: Option<String>,

    #[clap(short = 'w', long)]
    #[clap(group = "bind")]
    pub bindpass: Option<String>,

    #[clap(short = 'H', long)]
    pub host: Option<String>,

    #[clap(short = 'I', long)]
    pub scrape_interval_seconds: Option<u64>,

    #[clap(short = 'e', long)]
    #[clap(value_enum)]
    pub enable_flags: Vec<ArgFlag>,

    #[clap(short = 'd', long)]
    #[clap(value_enum)]
    pub disable_flags: Vec<ArgFlag>,
}
