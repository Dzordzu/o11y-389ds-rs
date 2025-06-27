use crate::haproxy;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeDisabled {
    /// Server is set to drainage
    pub mark_drain: bool,

    /// Maintenance started, but connections are still present
    pub mark_soft_maint: bool,

    /// Maintenance started, no connections were reported for a while
    pub mark_hard_maint: bool,

    /// Server stopped, no connections are allowed
    pub mark_stopped: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LdapStatus {
    pub is_systemd_running: bool,
    pub is_reachable: bool,
    pub connection_number: Option<u64>,
    pub queries_status: HashMap<String, bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Health {
    pub disabled: NodeDisabled,
    pub status: LdapStatus,
}

impl Default for Health {
    fn default() -> Self {
        Self::new()
    }
}

impl Health {
    pub fn new() -> Self {
        Health {
            disabled: NodeDisabled {
                mark_drain: false,
                mark_soft_maint: false,
                mark_hard_maint: false,
                mark_stopped: false,
            },
            status: LdapStatus {
                is_systemd_running: false,
                is_reachable: false,
                connection_number: None,
                queries_status: HashMap::new(),
            },
        }
    }

    /// to_haproxy_string errors
    fn _ths_errors(&self, response: &mut haproxy::Response, recover: &mut bool) {
        let failed_queries =
            self.status
                .queries_status
                .iter()
                .fold(None, |acc, (query, status)| {
                    if !status {
                        if let Some(acc) = acc {
                            Some(format!("{}, {}", acc, query))
                        } else {
                            Some(query.to_string())
                        }
                    } else {
                        acc
                    }
                });
        if let Some(failed_queries) = failed_queries {
            *recover = false;
            response.fail(Some(&format!(
                "heathcheck queries failed: {}",
                failed_queries
            )));
        }

        if !self.status.is_reachable {
            *recover = false;
            response.fail(Some("ldap is not reachable"));
        }

        if !self.status.is_systemd_running {
            *recover = false;
            response.fail(Some("dirsrv@default systemd unit is not running"));
        }
    }

    pub fn evaluate(&self, response: &mut haproxy::Response) {
        let mut recover = true;

        // Allow errors to override drain status
        if self.disabled.mark_drain {
            response.drain();
            recover = false;
        }

        // Allow errors in case of soft maintenance
        if self.disabled.mark_soft_maint {
            response.maintenance();
            recover = false;
        }

        self._ths_errors(response, &mut recover);

        // Skip errors in case of hard maintenance
        if self.disabled.mark_hard_maint {
            response.maintenance();
            recover = false;
        }

        // Skip errors in case of stopped
        if self.disabled.mark_stopped {
            response.stopped(Some("server stopped by operator"));
            recover = false;
        }

        if recover {
            response.up_and_ready();
        }
    }
}
