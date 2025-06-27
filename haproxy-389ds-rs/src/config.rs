use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_expose_port() -> u16 {
    6699
}

fn default_expose_address() -> String {
    "0.0.0.0".to_string()
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub haproxy: HaproxyConfig,

    #[serde(flatten)]
    pub common: internal::config::CommonConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ScrapeIntervalSeconds {
    pub replication_status: u64,
    pub ldap_monitoring: u64,
    pub systemd_status: u64,
    pub ldap_accessibility: u64,
}

impl Default for ScrapeIntervalSeconds {
    fn default() -> Self {
        Self {
            replication_status: 5,
            ldap_monitoring: 5,
            systemd_status: 5,
            ldap_accessibility: 5,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct HaproxyConfig {
    #[serde(default = "default_expose_port")]
    pub expose_port: u16,

    #[serde(default = "default_expose_address")]
    pub expose_address: String,

    #[serde(default)]
    pub scrape_interval_seconds: ScrapeIntervalSeconds,

    #[serde(default)]
    pub scrape_flags: ScrapeFlags,
}

impl Default for HaproxyConfig {
    fn default() -> Self {
        Self {
            expose_port: default_expose_port(),
            expose_address: default_expose_address(),
            scrape_interval_seconds: ScrapeIntervalSeconds::default(),
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

    #[serde(default = "default_true")]
    /// Check systemd unit status
    pub systemd_status: bool,
}

impl Default for ScrapeFlags {
    fn default() -> Self {
        Self {
            ldap_monitoring: true,
            replication_status: true,
            systemd_status: true,
        }
    }
}
