use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use ldap3::{Ldap, Scope, SearchEntry};

const UNKNOWN: &str = "UNKNOWN";

const SNMP_METRICS_INT: &[&str] = &[
    "anonymousbinds",
    "unauthbinds",
    "simpleauthbinds",
    "strongauthbinds",
    "bindsecurityerrors",
    "inops",
    "readops",
    "compareops",
    "addentryops",
    "removeentryops",
    "modifyentryops",
    "modifyrdnops",
    "listops",
    "searchops",
    "onelevelsearchops",
    "wholesubtreesearchops",
    "referrals",
    "chainings",
    "securityerrors",
    "errors",
    "connections",
    "connectionseq",
    "connectionsinmaxthreads",
    "connectionsmaxthreadscount",
    "bytesrecv",
    "bytessent",
    "entriesreturned",
    "referralsreturned",
    "supplierentries",
    "copyentries",
    "cacheentries",
    "cachehits",
    "consumerhits",
];

const ROOT_METRICS_INT: &[&str] = &[
    "threads",
    "currentconnections",
    "totalconnections",
    "currentconnectionsatmaxthreads",
    "maxthreadsperconnhits",
    "dtablesize",
    "readwaiters",
    "opsinitiated",
    "opscompleted",
    "entriessent",
    "bytessent",
    "nbackends",
];

const ROOT_METRICS_DATE: &[&str] = &["currenttime", "starttime"];

const DISK_METRICS_INT: &[&str] = &["used", "available", "size", "use%"];

#[derive(Debug, Default, Clone)]
pub struct MetricsCommonData {
    ///  List of the used connection dns over duration of the exporter process
    pub connections_dns: Vec<String>,

    ///  List of the used connection dns over duration of the exporter process
    pub connections_ips: Vec<String>,
}

#[derive(Debug)]
pub struct LdapConnection {
    pub dn: String,
    pub ip: String,
}

pub struct LdapConnections(Vec<LdapConnection>);
impl LdapConnections {
    pub fn count(&self) -> usize {
        self.0.len()
    }

    pub fn vec(&self) -> &Vec<LdapConnection> {
        &self.0
    }

    pub fn consume_vec(self) -> Vec<LdapConnection> {
        self.0
    }

    pub fn group_by_dn(&self) -> HashMap<String, u64> {
        self.0.iter().fold(HashMap::new(), |mut acc, x| {
            let v = acc.entry(x.dn.clone()).or_insert(0);
            *v += 1;
            acc
        })
    }

    pub fn group_by_ip(&self) -> HashMap<String, u64> {
        self.0.iter().fold(HashMap::new(), |mut acc, x| {
            let v = acc.entry(x.ip.clone()).or_insert(0);
            *v += 1;
            acc
        })
    }
}

/// Scrapable object
pub struct LdapMonitor {
    /// Version of the 389ds dirsrv
    pub version: String,

    /// List of active connections to the 389ds
    pub connections: LdapConnections,

    pub int_metrics: HashMap<String, u64>,
    pub date_metrics: HashMap<String, NaiveDateTime>,
}

impl LdapMonitor {
    pub async fn scrape(ldap: &mut Ldap) -> Result<Self> {
        let mut attrs = vec!["version", "connection"];
        attrs.extend(ROOT_METRICS_INT);
        attrs.extend(ROOT_METRICS_DATE);

        let search_int = ldap
            .search("cn=monitor", Scope::Base, "(objectClass=top)", attrs)
            .await?;

        if let Some(entry) = search_int.success()?.0.into_iter().next() {
            let entry = SearchEntry::construct(entry);

            let mut result = Self {
                version: Default::default(),
                connections: LdapConnections(Default::default()),
                int_metrics: Default::default(),
                date_metrics: Default::default(),
            };

            for (attr, attr_val) in entry.attrs {
                match attr.as_str() {
                    "version" => {
                        result.version = attr_val.first().cloned().unwrap_or_default();
                    }
                    "connection" => {
                        for attr in attr_val {
                            let values = attr.split(':').collect::<Vec<_>>();
                            let dn = values.get(5).unwrap_or(&UNKNOWN);
                            let ip = values.get(10).unwrap_or(&UNKNOWN).replace("ip=", "");

                            let connection = LdapConnection {
                                dn: dn.to_string(),
                                ip: ip.to_string(),
                            };
                            result.connections.0.push(connection);
                        }
                    }
                    _ if ROOT_METRICS_DATE.contains(&attr.as_str()) => {
                        if let Some(value) = attr_val.first() {
                            result.date_metrics.insert(
                                attr.clone(),
                                NaiveDateTime::parse_from_str(value, "%Y%m%d%H%M%SZ")?,
                            );
                        }
                    }
                    _ if ROOT_METRICS_INT.contains(&attr.as_str()) => {
                        if let Some(value) = attr_val.first() {
                            result
                                .int_metrics
                                .insert(attr.clone(), value.parse::<u64>()?);
                        }
                    }
                    _ => {}
                }
            }

            Ok(result)
        } else {
            Err(anyhow!("Unable to get root monitor metrics"))
        }
    }
}

pub struct LdapPartition {
    pub int_metrics: HashMap<String, u64>,
}

/// Scrapable object
pub struct LdapDisk {
    pub partitions: HashMap<String, LdapPartition>,
}

impl LdapDisk {
    pub async fn scrape(ldap: &mut Ldap) -> Result<Self> {
        let attrs = vec!["dsdisk"];

        #[allow(non_snake_case)]
        let ZER0: String = String::from("0");

        let search_int = ldap
            .search(
                "cn=disk space,cn=monitor",
                Scope::Base,
                "(objectClass=top)",
                attrs,
            )
            .await?;

        if let Some(entry) = search_int.success()?.0.into_iter().next() {
            let entry = SearchEntry::construct(entry);
            let mut result = Self {
                partitions: Default::default(),
            };

            for attr_vals in entry.attrs.values() {
                for attr_val in attr_vals {
                    let pairs: HashMap<String, String> = crate::logfmt::parse(attr_val).into();
                    if let Some(partition) = pairs.get("partition") {
                        let mut ldap_partition = LdapPartition {
                            int_metrics: Default::default(),
                        };

                        for key in DISK_METRICS_INT {
                            ldap_partition.int_metrics.insert(
                                key.to_string(),
                                pairs
                                    .get(*key)
                                    .unwrap_or(&ZER0)
                                    .parse::<u64>()
                                    .unwrap_or_default(),
                            );
                        }

                        result
                            .partitions
                            .insert(partition.to_string(), ldap_partition);
                    }
                }
            }
            Ok(result)
        } else {
            Err(anyhow!("Unable to get disk metrics"))
        }
    }
}

/// Scrapable object
pub struct LdapSNMP {
    pub int_metrics: HashMap<String, i32>,
}

impl LdapSNMP {
    pub async fn scrape(ldap: &mut Ldap) -> Result<Self> {
        let attrs = SNMP_METRICS_INT.to_vec();

        let search_int = ldap
            .search(
                "cn=snmp,cn=monitor",
                Scope::Base,
                "(objectClass=top)",
                attrs,
            )
            .await?;

        if let Some(entry) = search_int.success()?.0.into_iter().next() {
            let mut result = Self {
                int_metrics: Default::default(),
            };
            let entry = SearchEntry::construct(entry);

            for (attr, attr_val) in entry.attrs {
                if let Some(value) = attr_val.first() {
                    result
                        .int_metrics
                        .insert(attr, value.parse::<i32>().unwrap_or_default());
                }
            }
            Ok(result)
        } else {
            Err(anyhow!("Unable to get snmp metrics"))
        }
    }
}
