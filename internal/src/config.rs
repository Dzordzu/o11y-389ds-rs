use crate::{cli::CommandConfig, query::CustomQuery, LdapConfig};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Scrapers {
    #[serde(default)]
    pub dsctl: CommandConfig,

    #[serde(default)]
    pub query: Vec<CustomQuery>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CommonConfig {
    #[serde(flatten)]
    pub ldap_config: LdapConfig,

    #[serde(default)]
    pub scrapers: Scrapers,
}
