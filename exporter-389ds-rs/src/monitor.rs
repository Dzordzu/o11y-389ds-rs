use std::collections::{HashMap, HashSet};

use anyhow::Result;
use internal::LdapConfig;
use ldap3::Ldap;
use metrics::{counter, describe_counter, describe_gauge, gauge};

#[derive(Debug, Default, Clone)]
pub struct MetricsCommonData {
    ///  List of the used connection dns over duration of the exporter process
    pub connections_dns: HashMap<String, u64>,

    ///  List of the used connection dns over duration of the exporter process
    pub connections_ips: HashMap<String, u64>,

    /// Set of already recorded versions
    pub version: HashSet<String>,

    /// Number of scrapes
    pub scrapes: u64,
}

fn count_scrapes(prefix: &str, to_inc: Option<&mut u64>) {
    let name = format!("{prefix}_scrape.count");
    let counter = counter!(name.clone());
    describe_counter!(
        name,
        "How many scrapes have happened from the beggining of the process"
    );
    counter.increment(1);

    if let Some(to_inc) = to_inc {
        *to_inc += 1;
    }
}

async fn get_root_metrics(ldap: &mut Ldap, common_data: &mut MetricsCommonData) -> Result<()> {
    const PREFIX: &str = "monitor.";

    let scraped = internal::monitor::LdapMonitor::scrape(ldap).await?;
    count_scrapes(PREFIX, Some(&mut common_data.scrapes));

    let gauge = gauge!(format!("{PREFIX}version"), "version" => scraped.version.clone());
    gauge.set(1);

    // Reset metrics for older versions
    for version in common_data
        .version
        .iter()
        .filter(|x| *x != &scraped.version)
    {
        let gauge = gauge!(format!("{PREFIX}version"), "version" => version.clone());
        gauge.set(0);
    }
    common_data.version.insert(scraped.version);

    let gauge = gauge!(format!("{PREFIX}connection.count"));
    gauge.set(scraped.connections.count() as f64);

    // Add known dns from the previous runs
    let mut by_dn = scraped.connections.group_by_dn();
    for common_dn in common_data.connections_dns.keys() {
        by_dn.entry(common_dn.clone()).or_insert(0);
    }
    for (key, value) in by_dn {
        let sum = common_data.connections_dns.entry(key.clone()).or_insert(0);
        *sum += value;

        let gauge = gauge!(format!("{PREFIX}connection.by_dn"), "dn" => key.clone());
        gauge.set(value as f64);

        let gauge = gauge!(format!("{PREFIX}connection.avg.by_dn"), "dn" => key.clone());
        describe_gauge!(
            format!("{PREFIX}connection.avg.by_dn"),
            format!("Average value of {PREFIX}connection.by_dn from every scrape")
        );
        gauge.set((*sum as f64) / (common_data.scrapes as f64));
    }

    // Add known ips from the previous runs
    let mut by_ip = scraped.connections.group_by_ip();
    for common_ip in common_data.connections_ips.keys() {
        by_ip.entry(common_ip.clone()).or_insert(0);
    }
    for (key, value) in by_ip {
        let sum = common_data.connections_ips.entry(key.clone()).or_insert(0);
        *sum += value;

        let gauge = gauge!(format!("{PREFIX}connection.by_ip"), "ip" => key.clone());
        gauge.set(value as f64);

        let gauge = gauge!(format!("{PREFIX}connection.avg.by_ip"), "ip" => key.clone());
        describe_gauge!(
            format!("{PREFIX}connection.avg.by_ip"),
            format!("Average value of {PREFIX}connection.by_ip from every scrape")
        );
        gauge.set((*sum as f64) / (common_data.scrapes as f64));
    }

    for (attr, value) in scraped.int_metrics {
        let gauge = gauge!(format!("{PREFIX}{attr}"));
        gauge.set(value as f64)
    }

    for (attr, value) in scraped.date_metrics {
        let gauge = gauge!(format!("{PREFIX}{attr}"));
        gauge.set(value.and_utc().timestamp() as f64)
    }

    Ok(())
}

async fn get_disk_metrics(ldap: &mut Ldap) -> Result<()> {
    const PREFIX: &str = "monitor.disk.";

    let scraped = internal::monitor::LdapDisk::scrape(ldap).await?;
    count_scrapes(PREFIX, None);

    for (partition, pvalue) in scraped.partitions {
        for (metric, value) in pvalue.int_metrics {
            let metric = metric.replace("%", ".percentage");
            let gauge = gauge!(format!("{PREFIX}{metric}"), "partition" => partition.clone());
            gauge.set(value as f64);
        }
    }

    Ok(())
}

async fn get_ldap_snmp_metrics(ldap: &mut Ldap) -> Result<()> {
    const PREFIX: &str = "monitor.snmp.";

    let scraped = internal::monitor::LdapSNMP::scrape(ldap).await?;
    count_scrapes(PREFIX, None);

    for (attr, value) in scraped.int_metrics {
        let gauge = gauge!(format!("{PREFIX}{attr}"));
        gauge.set(value as f64);
    }

    Ok(())
}

pub async fn get_ldap_metrics(
    ldap_config: &LdapConfig,
    common_data: &mut MetricsCommonData,
) -> Result<()> {
    let mut ldap = ldap_config.connect().await?;

    get_root_metrics(&mut ldap, common_data).await?;
    get_disk_metrics(&mut ldap).await?;
    get_ldap_snmp_metrics(&mut ldap).await?;

    Ok(())
}
