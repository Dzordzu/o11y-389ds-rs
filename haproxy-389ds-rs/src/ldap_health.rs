use std::collections::HashMap;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct LdapStatus {
    pub is_systemd_running: bool,
    pub is_reachable: bool,
    pub connection_number: u64,
    pub queries_status: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct Health {
    pub disabled: NodeDisabled,
    pub status: LdapStatus,
}
