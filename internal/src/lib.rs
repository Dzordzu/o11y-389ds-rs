pub mod cli;
pub mod gids;
pub(crate) mod logfmt;
pub mod monitor;
pub mod query;
pub mod replica;

use anyhow::{anyhow, Result};
use ldap3::{Ldap, LdapConnAsync, Scope, SearchEntry};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bind {
    pub dn: String,
    pub pass: String,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LdapConfig {
    #[serde(default = "default_host", rename = "ldap_uri")]
    pub uri: String,

    #[serde(default = "default_true")]
    pub verify_certs: bool,

    #[serde(default = "default_page_size")]
    pub page_size: i32,

    #[serde(default, rename = "default_query_base")]
    pub default_base: String,

    pub bind: Option<Bind>,
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            bind: None,
            verify_certs: true,
            uri: default_host(),
            page_size: default_page_size(),
            default_base: Default::default(),
        }
    }
}

fn default_host() -> String {
    "ldap://localhost".to_string()
}

fn default_page_size() -> i32 {
    999
}

impl LdapConfig {
    pub async fn detect_base(&mut self) -> Result<()> {
        let (conn, mut ldap) = LdapConnAsync::new(&self.uri).await?;
        ldap3::drive!(conn);

        let result = ldap
            .search("", Scope::Base, "(objectClass=*)", &["namingContexts"])
            .await?;

        if let Some(first) = result.0.into_iter().next() {
            let entry = SearchEntry::construct(first);
            self.default_base = entry
                .attrs
                .get("namingContexts")
                .ok_or(anyhow!("No naming contexts attribute"))?
                .first()
                .ok_or(anyhow!("No naming contexts"))?
                .to_string();
            Ok(())
        } else {
            Err(anyhow!("Cannot retrive naming contexts"))
        }
    }

    pub async fn connect(&self) -> Result<Ldap> {
        let settings = ldap3::LdapConnSettings::new().set_no_tls_verify(!self.verify_certs);

        let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &self.uri).await?;
        ldap3::drive!(conn);

        if let Some(bind) = &self.bind {
            ldap.simple_bind(&bind.dn, &bind.pass).await?;
        }

        Ok(ldap)
    }
}
