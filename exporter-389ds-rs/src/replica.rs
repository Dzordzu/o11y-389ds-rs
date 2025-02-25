use std::collections::HashSet;

use anyhow::Result;
use internal::LdapConfig;
use ldap3::Ldap;
use metrics::{describe_gauge, gauge};

#[derive(Debug, Default, Clone)]
pub struct ReplicationCommonData {
    pub agreements: HashSet<String>,
}
async fn get_agreement_metrics(
    ldap: &mut Ldap,
    common_data: &mut ReplicationCommonData,
) -> Result<()> {
    const PREFIX: &str = "replication.";

    let scraped = internal::replica::Agreement::scrape(ldap).await?;

    let mut active_cns = HashSet::new();
    for entry in scraped {
        let labels = [
            ("agreement", entry.cn.clone()),
            ("host", entry.host),
            ("root", entry.root),
        ];

        active_cns.insert(entry.cn.clone());
        let g = gauge!(format!("{PREFIX}agreement"), &labels);
        g.set(1);

        for ruv in entry.ruvs {
            let mut ruv_labels = ruv.to_labels();
            ruv_labels.extend(
                labels
                    .iter()
                    .map(|x| (x.0.to_string(), x.1.clone()))
                    .collect::<Vec<(String, String)>>(),
            );
            let g = gauge!(format!("{PREFIX}agreement.ruv"), &ruv_labels);
            g.set(ruv.get_replica_id() as f64);
        }

        let g = gauge!(
            format!("{PREFIX}agreement.last_update_duration_seconds"),
            &labels
        );
        g.set(entry.last_update_duration_seconds as f64);

        for change in entry.changes_sent {
            let mut change_labels = vec![("replica_id", change.replica_id.to_string())];
            change_labels.extend(labels.clone());

            let g_replayed = gauge!(
                format!("{PREFIX}agreement.changes_replayed"),
                &change_labels
            );
            g_replayed.set(change.changes_replayed as f64);

            let g_skipped = gauge!(format!("{PREFIX}agreement.changes_skipped"), &change_labels);
            g_skipped.set(change.changes_skipped as f64);
        }

        let status = entry.status;

        let mut status_labels = vec![("state", status.state.to_string())];
        status_labels.extend(labels.clone());

        let g_ldap_status = gauge!(format!("{PREFIX}agreement.ldap_status"), &status_labels);
        g_ldap_status.set(status.ldap_rc as f64);

        let g_repl_status = gauge!(format!("{PREFIX}agreement.repl_status"), &status_labels);
        g_repl_status.set(status.repl_rc as f64);

        let date = status.date.and_utc().timestamp();
        let g_last_status_color = gauge!(
            format!("{PREFIX}agreement.last_status_color"),
            &status_labels
        );
        g_last_status_color.set(date as f64);
    }

    for agreement in common_data.agreements.difference(&active_cns) {
        let g = gauge!(format!("{PREFIX}agreements"), "agreement" => agreement.clone());
        g.set(0);
    }

    common_data.agreements.extend(active_cns);

    Ok(())
}

async fn get_replica_metrics(ldap: &mut Ldap) -> Result<()> {
    const PREFIX: &str = "replication.replica.";

    let scraped = internal::replica::Replica::scrape(ldap).await?;

    for entry in scraped {
        let labels = [("replica_root", entry.root), ("replica_name", entry.name)];

        let replica_replicareapactive = gauge!(format!("{PREFIX}replica_reap_active"), &labels);
        replica_replicareapactive.set(entry.currently_active_replication as u8 as f64);
        describe_gauge!(
            format!("{PREFIX}replica_reap_active"),
            format!("LDAP attribute: {}", internal::replica::REPLICA_ACTIVE)
        );

        let replica_change_count = gauge!(format!("{PREFIX}change_count"), &labels);
        replica_change_count.set(entry.changes_count as f64);
        describe_gauge!(
            format!("{PREFIX}change_count"),
            format!("LDAP attribute: {}", internal::replica::REPLICA_CHANGES)
        );
    }

    Ok(())
}

pub async fn get_ldap_replica_metrics(
    ldap_config: &LdapConfig,
    common_data: &mut ReplicationCommonData,
) -> Result<()> {
    const PREFIX: &str = "replication.";

    let mut ldap = ldap_config.connect().await?;

    let version = internal::replica::replication_plugin_version(&mut ldap).await?;
    let g = gauge!(format!("{PREFIX}plugin.version"), "version" => version.to_string());
    g.set(1);

    get_replica_metrics(&mut ldap).await?;
    get_agreement_metrics(&mut ldap, common_data).await?;

    Ok(())
}
