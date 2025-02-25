use serde::{de::Error, Deserialize, Deserializer};

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDateTime;
use ldap3::{Ldap, Scope, SearchEntry};
use serde_aux::prelude::*;

const SPACE: &str = " ";
const UNKNOWN: &str = "UNKNOWN";
const EMPTY_VEC_STR: &Vec<String> = &Vec::new();

const CN: &str = "cn";
const HOST: &str = "nsDS5ReplicaHost";
const ROOT: &str = "nsDS5ReplicaRoot";
const RUV: &str = "nsds50ruv";
const STATUS: &str = "nsds5replicaLastUpdateStatusJSON";

const UPDATE_START: &str = "nsds5replicaLastUpdateStart";
const UPDATE_END: &str = "nsds5replicaLastUpdateEnd";
const CHANGES_SENT: &str = "nsds5replicaChangesSentSinceStartup";

const REPLICA_ROOT: &str = "nsDS5ReplicaRoot";
const REPLICA_NAME: &str = "nsDS5ReplicaName";
pub const REPLICA_CHANGES: &str = "nsds5ReplicaChangeCount";
pub const REPLICA_ACTIVE: &str = "nsds5replicareapactive";

pub fn get_attr(entry: &SearchEntry, attr: &str) -> String {
    entry
        .attrs
        .get(attr)
        .unwrap_or(EMPTY_VEC_STR)
        .first()
        .cloned()
        .unwrap_or(UNKNOWN.to_string())
}

fn date_from_str<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ").map_err(D::Error::custom)
}

#[derive(serde::Deserialize, Debug)]
pub struct StatusJSON {
    pub state: String,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub ldap_rc: i64,
    pub ldap_rc_text: String,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub repl_rc: i64,
    pub repl_rc_text: String,

    #[serde(deserialize_with = "date_from_str")]
    pub date: NaiveDateTime,
    pub message: String,
}

pub struct ChangesSent {
    pub replica_id: i64,
    pub changes_replayed: u64,
    pub changes_skipped: u64,
}

impl ChangesSent {
    pub fn parse(definition: &str) -> Vec<Self> {
        definition
            .split(SPACE)
            .filter_map(|definition: &str| {
                let (replica_id, changes) = definition.split_once(":")?;
                let (changes_sent, changes_skipped) = changes.split_once("/")?;

                let replica_id = replica_id.parse().ok()?;
                let changes_sent = changes_sent.parse().ok()?;
                let changes_skipped = changes_skipped.parse().ok()?;

                Some(ChangesSent {
                    replica_id,
                    changes_replayed: changes_sent,
                    changes_skipped,
                })
            })
            .collect()
    }
}

pub enum Ruv {
    ReplicaGen(String),
    BrokenReplication {
        replica_id: i64,
        server: String,
    },
    Info {
        replica_id: i64,
        server: String,
        last_change: String,
        first_change: String,
    },
}

impl Ruv {
    pub fn to_labels(&self) -> Vec<(String, String)> {
        match self {
            Ruv::ReplicaGen(x) => vec![("replicagen".to_string(), x.clone())],
            Ruv::BrokenReplication {
                replica_id: _,
                server,
            } => vec![("server".to_string(), server.clone())],
            Ruv::Info {
                replica_id: _,
                server,
                last_change,
                first_change,
            } => vec![
                ("server".to_string(), server.clone()),
                ("last_change".to_string(), last_change.clone()),
                ("first_change".to_string(), first_change.clone()),
            ],
        }
    }

    pub fn get_replica_id(&self) -> i64 {
        match self {
            Ruv::ReplicaGen(_) => -1,
            Ruv::Info { replica_id, .. } => *replica_id,
            Ruv::BrokenReplication { replica_id, .. } => *replica_id,
        }
    }

    pub fn parse(definition: &str) -> Result<Self> {
        const REPLICAGEN: &str = "replicageneration";
        const REPLICA_PREFIX: &str = "replica ";

        let definition = definition.trim();
        let opening_bracket = definition
            .find('{')
            .ok_or(anyhow!("Missing opening bracket"))?
            + 1; // It's at the beginning...
        let (_, definition) = definition.split_at(opening_bracket);

        let closing_bracket = definition
            .find('}')
            .ok_or(anyhow!("Missing closing bracket"))?
            + 1;
        let (definition, changes) = definition.split_at(closing_bracket);
        let definition = definition.strip_suffix("}").unwrap();

        if definition == REPLICAGEN {
            return Ok(Ruv::ReplicaGen(changes.trim().to_string()));
        }

        // REPLICA_PREFIX needs to contain space in order to achieve
        // proper parsing
        if !REPLICA_PREFIX.ends_with(SPACE) {
            unreachable!("This should be handled in prefix!");
        }

        if !definition.starts_with(REPLICA_PREFIX) {
            return Err(anyhow!(
                "Broken definition: missing replica declaration in line {}",
                definition
            ));
        }

        let (_, definition) = definition.split_once(SPACE).unwrap();
        let (replica_id, server) = definition
            .split_once(SPACE)
            .ok_or(anyhow!("Broken definition: missing replica id"))?;

        let replica_id = replica_id
            .parse::<i64>()
            .context("Pasing replica id failed")?;
        let server = server.trim().to_string();

        // -------

        if let Some((last_change, first_change)) = changes.split_once(SPACE) {
            let (last_change, first_change) = (
                last_change.trim().to_string(),
                first_change.trim().to_string(),
            );

            Ok(Ruv::Info {
                replica_id,
                server,
                last_change,
                first_change,
            })
        } else {
            Ok(Ruv::BrokenReplication { replica_id, server })
        }
    }
}

/// Get version of the replica plugin
pub async fn replication_plugin_version(ldap: &mut Ldap) -> Result<String> {
    const ATTR: &str = "nsslapd-pluginversion";

    let attrs = vec![ATTR];
    let search = ldap
        .search(
            "cn=plugins,cn=config",
            Scope::Subtree,
            "(&(objectClass=nsslapdPlugin)(cn=*Replication*))",
            attrs,
        )
        .await?;

    let result = SearchEntry::construct(
        search
            .0
            .first()
            .ok_or(anyhow!("Could not get replication plugin entry in config"))?
            .clone(),
    );

    let version = result
        .attrs
        .get(ATTR)
        .ok_or(anyhow!("Could not get replication plugin version"))?
        .first()
        .ok_or(anyhow!("Replication plugin version seems to be empty"))?;

    Ok(version.to_string())
}

pub struct Agreement {
    pub cn: String,
    pub host: String,
    pub root: String,

    pub changes_sent: Vec<ChangesSent>,
    pub last_update_duration_seconds: i64,

    pub ruvs: Vec<Ruv>,
    pub status: StatusJSON,
}

impl Agreement {
    pub async fn scrape(ldap: &mut Ldap) -> Result<Vec<Self>> {
        let attrs = vec![
            CN,
            HOST,
            ROOT,
            RUV,
            UPDATE_START,
            UPDATE_END,
            CHANGES_SENT,
            STATUS,
        ];

        let search = ldap
            .search(
                "cn=config",
                Scope::Subtree,
                "(objectClass=nsds5ReplicationAgreement)",
                attrs,
            )
            .await?;

        let mut result = Vec::new();

        for entry in search.0 {
            let entry = SearchEntry::construct(entry);

            let cn = get_attr(&entry, CN);
            let host = get_attr(&entry, HOST);
            let root = get_attr(&entry, ROOT);

            let update_start = get_attr(&entry, UPDATE_START);
            let update_end = get_attr(&entry, UPDATE_END);
            let changes_sent = get_attr(&entry, CHANGES_SENT);
            let status = get_attr(&entry, STATUS);

            let mut ruvs = Vec::<Ruv>::new();
            for ruv in entry.attrs.get(RUV).unwrap_or(EMPTY_VEC_STR) {
                ruvs.push(Ruv::parse(ruv)?)
            }

            let update_start = NaiveDateTime::parse_from_str(&update_start, "%Y%m%d%H%M%SZ")?;
            let update_end = NaiveDateTime::parse_from_str(&update_end, "%Y%m%d%H%M%SZ")?;
            let last_update_duration_seconds = (update_start - update_end).num_seconds();

            let changes_sent = ChangesSent::parse(&changes_sent);
            let status: StatusJSON = serde_json::from_str(&status)?;

            result.push(Agreement {
                cn,
                host,
                root,
                changes_sent,
                last_update_duration_seconds,
                ruvs,
                status,
            })
        }
        Ok(result)
    }
}

pub struct Replica {
    pub root: String,
    pub name: String,
    pub changes_count: u64,
    pub currently_active_replication: bool,
}

impl Replica {
    pub async fn scrape(ldap: &mut Ldap) -> Result<Vec<Self>> {
        let attrs = vec![REPLICA_ROOT, REPLICA_NAME, REPLICA_CHANGES, REPLICA_ACTIVE];
        let search = ldap
            .search(
                "cn=config",
                Scope::Subtree,
                "(objectClass=nsds5replica)",
                attrs,
            )
            .await?;

        let mut result = Vec::new();
        for entry in search.0 {
            let entry = SearchEntry::construct(entry);

            let root = get_attr(&entry, REPLICA_ROOT);
            let name = get_attr(&entry, REPLICA_NAME);
            let changes = get_attr(&entry, REPLICA_CHANGES);
            let active = get_attr(&entry, REPLICA_ACTIVE);

            let changes_count = changes.parse::<u64>()?;
            let currently_active_replication = active.parse::<u8>()? != 0;

            result.push(Replica {
                root,
                name,
                changes_count,
                currently_active_replication,
            })
        }

        Ok(result)
    }
}
