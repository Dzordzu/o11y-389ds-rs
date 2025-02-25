use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Copy)]
pub enum Severity {
    HIGH = 1000,
    MEDIUM = 100,
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

#[derive(Debug, Deserialize, Clone)]
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
            timeout(Duration::from_secs(timeout_s), cmd.output()).await??
        } else {
            cmd.output().await?
        })
    }

    pub async fn healthcheck(&self) -> Result<Vec<HealthcheckEntry>> {
        let mut cmd = Command::new("dsctl");
        cmd.args(["--json", &self.instance_name, "healthcheck"]);

        let result = self.execute_cmd(&mut cmd).await?;

        if !result.status.success() {
            return Err(anyhow!("dsctl healthcheck failed"));
        }

        Ok(serde_json::from_slice(&result.stdout)?)
    }
}
