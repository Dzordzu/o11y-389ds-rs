use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

pub fn calculate_dir_checksum(path: &Path) -> Result<String> {
    if !path.is_dir() {
        let binary_content = std::fs::read(path)?;

        let mut hasher = Sha256::new();
        hasher.update(&binary_content);
        Ok(format!("{:x}", hasher.finalize()))
    } else {
        let mut result = String::new();

        for entry in path.read_dir()? {
            result += &calculate_dir_checksum(&entry?.path())?;
        }

        let mut hasher = Sha256::new();
        hasher.update(&result);
        Ok(format!("{:x}", hasher.finalize()))
    }
}
