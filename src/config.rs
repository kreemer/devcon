// MIT License
//
// Copyright (c) 2025 DevCon Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! # Configuration Module
//!
//! This module handles loading and managing DevCon configuration files.
//!
//! ## Overview
//!
//! The configuration is stored in YAML format in the XDG config directory,
//! typically at `~/.config/devcon/config.yaml` on Linux/macOS.
//!
//! ## Configuration Options
//!
//! - **dotfiles_repository** - URL to a dotfiles repository to clone into containers
//! - **additional_features** - List of devcontainer features to add to all containers
//! - **env_variables** - Environment variables to pass to all containers
//!
//! ## Examples
//!
//! ```yaml
//! dotfiles_repository: https://github.com/user/dotfiles
//! additional_features:
//!   ghcr.io/devcontainers/features/common-utils:2:
//!     installZsh: true
//! env_variables:
//!   - EDITOR=vim
//!   - LANG=en_US.UTF-8
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Main configuration structure for DevCon.
///
/// This structure holds user preferences and defaults that are applied
/// across all devcontainer operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// URL to a dotfiles repository.
    ///
    /// If set, this repository will be cloned into the container
    /// to provide user-specific configuration files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotfiles_repository: Option<String>,

    /// Install command which will be used to install dotfiles
    ///
    /// If set, this command will be used to install the dotfiles after cloning
    /// If unset, will search for common install scripts like install.sh, setup.sh, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotfiles_install_command: Option<String>,

    /// Default shell
    ///
    /// If set, the shell command will use this shell to exec into the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_shell: Option<String>,

    /// Additional devcontainer features to include in all containers.
    ///
    /// These features are merged with features defined in devcontainer.json.
    /// The key is the feature identifier (e.g., "ghcr.io/owner/repo/feature:version")
    /// and the value is a map of options for that feature.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub additional_features: HashMap<String, serde_json::Value>,

    /// Environment variables to pass to containers.
    ///
    /// If the string has the format KEY=value, it will be set as an environment variable in the container
    /// If its only a string without "=" it will be passed through as is from the host container.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_variables: Vec<String>,

    /// Container runtime to use.
    ///
    /// Valid values: "auto", "docker", "apple"
    /// If set to "auto" (default), the runtime will be auto-detected.
    #[serde(
        default = "default_runtime",
        skip_serializing_if = "is_default_runtime"
    )]
    pub runtime: String,
}

fn default_runtime() -> String {
    "auto".to_string()
}

fn is_default_runtime(runtime: &str) -> bool {
    runtime == "auto"
}

impl Config {
    /// Loads the configuration from the XDG config directory.
    ///
    /// This method looks for the config file at:
    /// - `$XDG_CONFIG_HOME/devcon/config.yaml` (if XDG_CONFIG_HOME is set)
    /// - `~/.config/devcon/config.yaml` (default on Linux/macOS)
    /// - `%APPDATA%/devcon/config.yaml` (on Windows)
    ///
    /// If no config file exists, returns a default empty configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config file exists but cannot be read
    /// - The config file contains invalid YAML
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::config::Config;
    /// let config = Config::load()?;
    /// if let Some(dotfiles) = &config.dotfiles_repository {
    ///     println!("Dotfiles repo: {}", dotfiles);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        Ok(config)
    }

    /// Saves the configuration to the XDG config directory.
    ///
    /// This method creates the config directory if it doesn't exist,
    /// then writes the configuration in YAML format.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config directory cannot be created
    /// - The config file cannot be written
    /// - The configuration cannot be serialized to YAML
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::config::Config;
    /// let mut config = Config::default();
    /// config.dotfiles_repository = Some("https://github.com/user/dotfiles".to_string());
    /// config.save()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let yaml = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize configuration to YAML")?;

        fs::write(&config_path, yaml)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Returns the path to the config file.
    ///
    /// This uses the XDG Base Directory specification on Unix-like systems
    /// and the appropriate AppData directory on Windows.
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be determined
    /// (e.g., if HOME is not set).
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to determine config directory")?;

        Ok(config_dir.join("devcon").join("config.yaml"))
    }

    /// Merges additional features from the config with existing features.
    ///
    /// This creates a combined map of features, with devcontainer.json
    /// features taking precedence over config features.
    ///
    /// # Arguments
    ///
    /// * `devcontainer_features` - Features from devcontainer.json
    ///
    /// # Returns
    ///
    /// A HashMap containing all features with their options.
    #[allow(dead_code)]
    pub fn merge_features(
        &self,
        devcontainer_features: &[(String, serde_json::Value)],
    ) -> HashMap<String, serde_json::Value> {
        let mut merged = self.additional_features.clone();

        // Devcontainer features override config features
        for (key, value) in devcontainer_features {
            merged.insert(key.clone(), value.clone());
        }

        merged
    }

    /// Detects which container runtime is available.
    ///
    /// Checks for Docker and Apple's container CLI in order.
    /// Returns "docker" if docker is available, "apple" if container is available,
    /// or an error if neither is found.
    pub fn detect_runtime() -> Result<String> {
        // Check for docker
        if Command::new("docker")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok("docker".to_string());
        }

        // Check for Apple container CLI
        if Command::new("container")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok("apple".to_string());
        }

        anyhow::bail!("No container runtime found. Please install Docker or Apple's container CLI.")
    }

    /// Gets the runtime to use, resolving "auto" to a specific runtime.
    pub fn resolve_runtime(&self) -> Result<String> {
        if self.runtime == "auto" {
            Self::detect_runtime()
        } else {
            Ok(self.runtime.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.dotfiles_repository.is_none());
        assert!(config.additional_features.is_empty());
        assert!(config.env_variables.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            dotfiles_repository: Some("https://github.com/user/dotfiles".to_string()),
            env_variables: vec!["EDITOR=vim".to_string()],
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("dotfilesRepository"));
        assert!(yaml.contains("https://github.com/user/dotfiles"));
        assert!(yaml.contains("envVariables"));
    }

    #[test]
    fn test_config_deserialization() {
        let yaml = r#"
dotfilesRepository: https://github.com/user/dotfiles
additionalFeatures:
  ghcr.io/devcontainers/features/git:1:
    version: latest
envVariables:
  - EDITOR=vim
  - LANG=en_US.UTF-8
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.dotfiles_repository,
            Some("https://github.com/user/dotfiles".to_string())
        );
        assert_eq!(config.additional_features.len(), 1);
        assert_eq!(config.env_variables.len(), 2);
    }

    #[test]
    fn test_merge_features() {
        let mut config = Config::default();
        config.additional_features.insert(
            "ghcr.io/devcontainers/features/git:1".to_string(),
            serde_json::json!({"version": "latest"}),
        );
        config.additional_features.insert(
            "ghcr.io/devcontainers/features/node:2".to_string(),
            serde_json::json!({"version": "18"}),
        );

        let devcontainer_features = vec![(
            "ghcr.io/devcontainers/features/node:2".to_string(),
            serde_json::json!({"version": "20"}),
        )];

        let merged = config.merge_features(&devcontainer_features);

        assert_eq!(merged.len(), 2);
        // Devcontainer feature should override config feature
        assert_eq!(
            merged.get("ghcr.io/devcontainers/features/node:2").unwrap()["version"],
            "20"
        );
        assert_eq!(
            merged.get("ghcr.io/devcontainers/features/git:1").unwrap()["version"],
            "latest"
        );
    }
}
