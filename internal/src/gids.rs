use std::collections::HashMap;

use crate::LdapConfig;
use anyhow::{anyhow, Result};
use ldap3::{
    adapters::{Adapter, EntriesOnly, PagedResults},
    Scope, SearchEntry,
};
use serde::Serialize;

const UID: &str = "uid";
const GID_NUMBER: &str = "gidNumber";
const ACCOUNTS_ATTRS: &[&str] = &[GID_NUMBER, UID];

type GidNumbers = Vec<i64>;

#[derive(Serialize, Debug)]
struct LdapAccount {
    pub dn: String,
    pub uid: String,
    pub gid_number: i64,
}

async fn load_accounts(ldap_config: &LdapConfig) -> Result<Vec<LdapAccount>> {
    let mut ldap = ldap_config.connect().await?;

    let adapters: Vec<Box<dyn Adapter<_, _>>> = vec![
        Box::new(EntriesOnly::new()),
        Box::new(PagedResults::new(ldap_config.page_size)),
    ];

    let mut search = ldap
        .streaming_search_with(
            adapters,
            &ldap_config.default_base,
            Scope::Subtree,
            "(objectClass=posixAccount)",
            ACCOUNTS_ATTRS,
        )
        .await?;

    let mut result = Vec::new();

    while let Some(entry) = search.next().await? {
        let entry = SearchEntry::construct(entry);

        #[allow(non_snake_case)]
        let DEF_UNKNOWN = vec![String::new()];

        let dn = entry.dn;
        let uid = entry
            .attrs
            .get(UID)
            .unwrap_or(&DEF_UNKNOWN)
            .first()
            .ok_or(anyhow::anyhow!("No UID attribute"))?
            .clone();

        let gid_number = entry
            .attrs
            .get(GID_NUMBER)
            .unwrap_or(&DEF_UNKNOWN)
            .first()
            .ok_or(anyhow::anyhow!("No GID attribute"))?
            .parse::<i64>()
            .unwrap();

        result.push(LdapAccount {
            dn,
            uid,
            gid_number,
        })
    }

    Ok(result)
}

async fn load_groups(ldap_config: &LdapConfig) -> Result<GidNumbers> {
    let mut ldap = ldap_config.connect().await?;

    let adapters: Vec<Box<dyn Adapter<_, _>>> = vec![
        Box::new(EntriesOnly::new()),
        Box::new(PagedResults::new(ldap_config.page_size)),
    ];

    let mut search = ldap
        .streaming_search_with(
            adapters,
            &ldap_config.default_base,
            Scope::Subtree,
            "(objectClass=posixGroup)",
            vec![GID_NUMBER],
        )
        .await?;

    let mut result = Vec::new();

    while let Some(entry) = search.next().await? {
        let entry = SearchEntry::construct(entry);

        #[allow(non_snake_case)]
        let DEF_UNKNOWN = vec![String::new()];

        let gid_number = entry
            .attrs
            .get(GID_NUMBER)
            .unwrap_or(&DEF_UNKNOWN)
            .first()
            .ok_or(anyhow!("No GID attribute"))?
            .parse::<i64>()
            .unwrap();

        result.push(gid_number);
    }

    Ok(result)
}

/// missing gid -> uid occurences number
fn missing_gids(accounts: &[LdapAccount], groups: &[i64]) -> HashMap<i64, u64> {
    accounts
        .iter()
        .filter(|account| !groups.contains(&account.gid_number))
        .fold(HashMap::new(), |mut acc, account| {
            let entry = acc.entry(account.gid_number).or_insert(0);
            *entry += 1;

            acc
        })
}

/// Get missing gid -> uid occurences number
pub async fn missing_gids_to_uid_mapping(ldap_config: &LdapConfig) -> Result<HashMap<i64, u64>> {
    let accounts = crate::gids::load_accounts(ldap_config);
    let groups = crate::gids::load_groups(ldap_config);

    let (accounts, groups) = tokio::join!(accounts, groups);
    let (accounts, groups) = (accounts?, groups?);

    Ok(missing_gids(&accounts, &groups))
}
