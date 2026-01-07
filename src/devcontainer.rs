use std::fs;
use std::path::PathBuf;

use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
pub struct Devcontainer {
    pub name: String,
    pub image: String,
    pub features: Vec<Feature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Feature {
    pub url: String,
    pub options: serde_json::Value,
}

impl Feature {
    pub fn get_registry(&self) -> anyhow::Result<String> {
        let parts: Vec<&str> = self.url.split('/').collect();
        if parts.is_empty() {
            bail!("Invalid feature URL")
        }
        Ok(parts.get(0).unwrap().to_string())
    }

    pub fn get_name(&self) -> anyhow::Result<String> {
        let mut parts: Vec<&str> = self.url.split('/').collect();

        let name_part = parts.pop();
        let name = name_part
            .unwrap()
            .split(':')
            .nth(0)
            .unwrap_or("unknown")
            .to_string();
        Ok(name)
    }

    pub fn get_version(&self) -> anyhow::Result<String> {
        let parts: Vec<&str> = self.url.split('/').collect();

        if parts[parts.len() - 1].contains(':') {
            let version_part = parts.clone().pop();
            let version = version_part.unwrap().split(':').nth(1).unwrap_or("latest");
            return Ok(version.to_string());
        }

        Ok("latest".to_string())
    }

    pub fn get_repository(&self) -> anyhow::Result<String> {
        let parts: Vec<&str> = self.url.split('/').collect();

        let mut repo_parts: Vec<&str> = Vec::new();
        for part in &parts {
            repo_parts.push(part);
        }
        repo_parts.pop();

        let repository = String::from(
            repo_parts
                .iter()
                .skip(1)
                .cloned()
                .collect::<Vec<&str>>()
                .join("/"),
        );

        Ok(repository)
    }
}

impl TryFrom<PathBuf> for Devcontainer {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let final_path = path.join(".devcontainer").join("devcontainer.json");
        if !fs::exists(&final_path).is_ok() {
            bail!(
                "Devcontainer definition not found in {}",
                &final_path.to_string_lossy()
            )
        }

        let file_result = fs::read_to_string(&final_path);

        if !file_result.is_ok() {
            bail!(
                "Devcontainer definition cannot be read {}",
                &final_path.to_string_lossy()
            )
        }

        let result = Self::try_from(file_result.unwrap());
        if result.is_err() {
            bail!("Devcontainer content could not be parsed")
        }

        Ok(result.unwrap())
    }
}

impl TryFrom<String> for Devcontainer {
    type Error = serde_json::Error;

    fn try_from(content: String) -> std::result::Result<Self, Self::Error> {
        serde_json::from_str(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_feature_registry() {
        let feature = Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: serde_json::Value::Null,
        };

        assert!(feature.get_registry().is_ok());
        assert_eq!("ghcr.io", feature.get_registry().unwrap());
    }
    #[test]
    fn test_parsing_feature_repository() {
        let feature = Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: serde_json::Value::Null,
        };

        assert!(feature.get_repository().is_ok());
        assert_eq!("devcontainers/features", feature.get_repository().unwrap());
    }
    #[test]
    fn test_parsing_feature_name() {
        let feature = Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: serde_json::Value::Null,
        };

        assert!(feature.get_name().is_ok());
        assert_eq!("github-cli", feature.get_name().unwrap());
    }
    #[test]
    fn test_parsing_feature_version() {
        let feature = Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: serde_json::Value::Null,
        };

        assert!(feature.get_version().is_ok());
        assert_eq!("1", feature.get_version().unwrap());
    }
}
