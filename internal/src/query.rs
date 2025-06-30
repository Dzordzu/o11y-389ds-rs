use sha2::{Digest, Sha256};
use std::time::Instant;

use anyhow::Result;
use ldap3::{
    adapters::{Adapter, EntriesOnly, PagedResults},
    Ldap, Scope, SearchEntry,
};
use serde::Deserialize;

use crate::Bind;

#[derive(Deserialize, Debug, Clone)]
pub struct CustomQuery {
    pub name: String,
    pub filter: String,
    pub max_entries: Option<i32>,

    #[serde(default)]
    pub attrs: Vec<String>,

    pub verify_certs: Option<bool>,
    pub bind: Option<Bind>,
    pub uri: Option<String>,
    pub page_size: Option<i32>,
    pub default_base: Option<String>,

    /// It's the operational parameter, handled by the code
    #[serde(skip, default)]
    pub ldap_config: Option<crate::LdapConfig>,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    /// Number of the returned dns
    pub object_count: u64,
    /// Number of distinctive pairs (dn, attr_name)
    pub attrs_count: u64,
    /// Duration of the query
    pub query_time: std::time::Duration,
    pub ldap_code: u32,
    pub sha256_checksum: String,

    /// Bytes of the received attributes values
    pub bytes: u64,
}

impl CustomQuery {
    /// Return a new instance
    pub fn new(name: String, filter: String, ldap_config: crate::LdapConfig) -> Self {
        Self {
            name,
            filter,
            max_entries: None,
            attrs: Vec::new(),
            bind: None,
            uri: None,
            page_size: None,
            default_base: None,
            verify_certs: None,
            ldap_config: Some(ldap_config),
        }
    }

    pub async fn connect(&self) -> Result<Ldap> {
        let mut config = self.ldap_config.clone().ok_or(anyhow::anyhow!(
            "No ldap config. This is (most likely) a bug"
        ))?;

        if let Some(uri) = self.uri.clone() {
            config.uri = uri;
        }

        if let Some(page_size) = self.page_size {
            config.page_size = page_size;
        }

        if let Some(default_base) = self.default_base.clone() {
            config.default_base = default_base;
        }

        if let Some(bind) = self.bind.clone() {
            config.bind = Some(bind);
        }

        if let Some(verify_certs) = self.verify_certs {
            config.verify_certs = verify_certs;
        }

        config.connect().await
    }
    pub async fn get_metrics(&self) -> Result<Metrics> {
        let mut ldap = self.connect().await?;

        let ldap_config = self.ldap_config.as_ref().ok_or(anyhow::anyhow!(
            "No ldap config. This is (most likely) a bug"
        ))?;

        let adapters: Vec<Box<dyn Adapter<_, _>>> = vec![
            Box::new(EntriesOnly::new()),
            Box::new(PagedResults::new(ldap_config.page_size)),
        ];

        if let Some(max_entries) = self.max_entries {
            ldap.with_search_options(ldap3::SearchOptions::new().sizelimit(max_entries));
        }

        let mut search = ldap
            .streaming_search_with(
                adapters,
                &ldap_config.default_base,
                Scope::Subtree,
                &self.filter,
                &self.attrs,
            )
            .await?;
        let mut object_count = 0;
        let mut attrs_count: u64 = 0;

        let mut checksums: Vec<(String, serde_json::Value)> = Vec::new();

        let mut bytes = 0_u64;

        let start = Instant::now();
        while let Some(entry) = search.next().await? {
            let entry = SearchEntry::construct(entry);

            bytes += entry.attrs.iter().fold(0, |acc, x| acc + x.1.len()) as u64;
            attrs_count += entry.attrs.len() as u64;

            println!("With attrs: {:?}: {:?}", self.attrs, entry);

            let mut attrs: Vec<(String, serde_json::Value)> = entry
                .attrs
                .into_iter()
                .map(|mut x| {
                    x.1.sort();
                    (x.0, serde_json::to_value(&x.1).unwrap())
                })
                .collect();

            attrs.sort_by_key(|x| x.0.clone());

            checksums.push((entry.dn.clone(), serde_json::to_value(attrs).unwrap()));

            object_count += 1;
        }
        let query_time = start.elapsed();

        let ldap_code = search.finish().await.rc;

        checksums.sort_by_key(|x| x.0.clone());

        let mut hasher = Sha256::new();
        hasher.update(
            checksums
                .into_iter()
                .fold(String::new(), |acc, x| format!("{acc}{}", x.1)),
        );
        let sha256_checksum = format!("{:x}", hasher.finalize());

        Ok(Metrics {
            object_count,
            attrs_count,
            query_time,
            ldap_code,
            sha256_checksum,
            bytes,
        })
    }
}
