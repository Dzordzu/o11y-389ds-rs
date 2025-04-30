use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub const DEFAULT_INSTANCE: &str = "default";

fn default_instance() -> String {
    DEFAULT_INSTANCE.to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandConfig {
    pub timeout_seconds: Option<u64>,

    #[serde(rename = "instance", default = "default_instance")]
    pub instance_name: String,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: None,
            instance_name: default_instance(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Copy, std::hash::Hash)]
pub enum Severity {
    #[serde(alias = "High", alias = "high", alias = "HIGH")]
    HIGH = 1000,
    #[serde(alias = "Medium", alias = "medium", alias = "MEDIUM")]
    MEDIUM = 100,
    #[serde(alias = "Low", alias = "low", alias = "LOW")]
    LOW = 0,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Severity::HIGH => "HIGH",
            Severity::MEDIUM => "MEDIUM",
            Severity::LOW => "LOW",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, std::hash::Hash)]
pub struct HealthcheckEntry {
    pub dsle: String,
    pub severity: Severity,
    pub items: Vec<String>,
    pub detail: String,
    pub fix: String,
    pub description: String,
}

impl CommandConfig {
    pub fn new(timeout_seconds: Option<u64>, instance_name: String) -> Self {
        Self {
            timeout_seconds,
            instance_name,
        }
    }

    async fn execute_cmd(&self, cmd: &mut Command) -> Result<std::process::Output> {
        Ok(if let Some(timeout_s) = self.timeout_seconds {
            timeout(
                Duration::from_secs(timeout_s),
                cmd.kill_on_drop(true).output(),
            )
            .await??
        } else {
            cmd.output().await?
        })
    }

    async fn list_checks(&self) -> Result<Vec<String>> {
        let mut cmd = Command::new("sudo");
        cmd.args([
            "dsctl",
            "--json",
            &self.instance_name,
            "healthcheck",
            "--list-checks",
        ]);
        let result = self.execute_cmd(&mut cmd).await?;
        if !result.status.success() {
            let error = std::str::from_utf8(&result.stderr)
                .unwrap_or("Undefined error. That is really bad");
            return Err(anyhow!("dsctl healthcheck --list-checks failed: {}", error));
        }

        let output = std::string::String::from_utf8(result.stdout)
            .map_err(|e| anyhow!("Response is not a valid UTF8 string: {}", e))?;

        let checks: HashSet<String> = output
            .lines()
            .filter_map(|x| x.split_once(":"))
            .map(|x| format!("{}:*", x.0))
            .collect();
        Ok(Vec::from_iter(checks))
    }

    pub async fn healthcheck(&self, check_pattern: &str) -> Result<Vec<HealthcheckEntry>> {
        let mut cmd = Command::new("sudo");
        cmd.args([
            "dsctl",
            "--json",
            &self.instance_name,
            "healthcheck",
            "--check",
            check_pattern,
        ]);

        let result = self.execute_cmd(&mut cmd).await?;

        if !result.status.success() {
            let error = std::str::from_utf8(&result.stderr)
                .unwrap_or("Undefined error. That is really bad");
            return Err(anyhow!("dsctl healthcheck failed: {}", error));
        }

        Ok(serde_json::from_slice(&result.stdout)?)
    }

    pub async fn healthchecks(&self) -> Result<Vec<HealthcheckEntry>> {
        let check_patterns = self.list_checks().await?;
        let mut result = Vec::new();

        for pattern in check_patterns
            .into_iter()
            // Skip logs for now as they can grow exponentially
            .filter(|x| !x.starts_with("logs"))
        {
            result.extend(self.healthcheck(&pattern).await?);
        }

        Ok(result)
    }
}
