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

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub recent_paths: Vec<PathBuf>,
    pub dotfiles_repo: Option<String>,
    pub additional_features: std::collections::HashMap<String, String>,
    pub env: Vec<AppConfigEnv>,
    pub socket_path: PathBuf,
}

impl AppConfig {
    pub fn list_env_by_context(&self, context: DevContainerContext) -> Vec<&AppConfigEnv> {
        self.env
            .iter()
            .filter(|env| env.context == context || env.context == DevContainerContext::All)
            .collect()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfigEnv {
    pub name: String,
    pub value: String,
    pub context: DevContainerContext,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DevContainerContext {
    #[serde(rename = "all")]
    #[default]
    All,
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "exec")]
    Exec,
}

impl FromStr for DevContainerContext {
    type Err = ();

    fn from_str(input: &str) -> Result<DevContainerContext, Self::Err> {
        match input.to_lowercase().trim() {
            "all" => Ok(DevContainerContext::All),
            "up" => Ok(DevContainerContext::Up),
            "exec" => Ok(DevContainerContext::Exec),
            _ => Err(()),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new(config_path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(ConfigManager { config_path })
    }

    pub fn load_or_create_config(&self) -> Result<AppConfig, Box<dyn std::error::Error>> {
        if !self.config_path.exists() {
            let default_config = AppConfig::default();
            self.save_config(&default_config)?;
            Ok(default_config)
        } else {
            self.load_config()
        }
    }

    pub fn load_config(&self) -> Result<AppConfig, Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string(&self.config_path)?;
        let config: AppConfig = serde_yaml::from_str(&config_content)?;
        Ok(config)
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure the config directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config_content = serde_yaml::to_string(config)?;
        fs::write(&self.config_path, config_content)?;
        Ok(())
    }

    pub fn add_recent_path(
        &self,
        mut config: AppConfig,
        path: PathBuf,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        // Convert to absolute path
        let abs_path = path.canonicalize()?;

        // Remove if already exists to avoid duplicates
        config.recent_paths.retain(|p| p != &abs_path);

        // Add at the beginning (most recent first)
        config.recent_paths.insert(0, abs_path);

        // Limit to 10 recent paths
        if config.recent_paths.len() > 10 {
            config.recent_paths.truncate(10);
        }

        // Save the updated config
        self.save_config(&config)?;

        Ok(config)
    }

    pub fn set_dotfiles_repo(
        &self,
        mut config: AppConfig,
        dotfiles_repo: Option<String>,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        config.dotfiles_repo = dotfiles_repo;
        self.save_config(&config)?;
        Ok(config)
    }

    pub fn add_feature(
        &self,
        mut config: AppConfig,
        feature_name: String,
        feature_value: String,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        config
            .additional_features
            .insert(feature_name, feature_value);
        self.save_config(&config)?;
        Ok(config)
    }

    pub fn remove_feature(
        &self,
        mut config: AppConfig,
        feature_name: String,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        if !config.additional_features.contains_key(&feature_name) {
            return Err(format!("Feature '{feature_name}' not found").into());
        }
        config.additional_features.remove(&feature_name);
        self.save_config(&config)?;
        Ok(config)
    }

    pub fn clear_features(
        &self,
        mut config: AppConfig,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        config.additional_features.clear();
        self.save_config(&config)?;
        Ok(config)
    }
    pub fn add_env(
        &self,
        mut config: AppConfig,
        env_name: String,
        env_value: String,
        env: Option<DevContainerContext>,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        config.env.push(AppConfigEnv {
            name: env_name,
            value: env_value,
            context: env.unwrap_or_default(),
        });
        self.save_config(&config)?;
        Ok(config)
    }

    pub fn remove_env(
        &self,
        mut config: AppConfig,
        index: usize,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        if index >= config.env.len() {
            return Err("Index out of bounds".into());
        }
        config.env.swap_remove(index);
        self.save_config(&config)?;
        Ok(config)
    }

    pub fn clear_env(
        &self,
        mut config: AppConfig,
    ) -> Result<AppConfig, Box<dyn std::error::Error>> {
        config.env.clear();
        self.save_config(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_overwriting_config_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        assert!(!config_manager.config_path.exists());
        let _ = config_manager
            .load_or_create_config()
            .expect("Failed to create config");
        assert!(config_manager.config_path.exists());
    }

    #[test]
    fn test_config_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        assert!(config.recent_paths.is_empty());
        temp_dir.close().unwrap();
    }

    #[test]
    fn test_add_recent_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let test_path = &temp_dir.path().to_path_buf();
        let updated_config = config_manager
            .add_recent_path(config, test_path.clone())
            .unwrap();

        assert_eq!(updated_config.recent_paths.len(), 1);
        assert_eq!(
            updated_config.recent_paths[0],
            test_path.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_duplicate_path_handling() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let test_path = &temp_dir.path().to_path_buf();
        let config = config_manager
            .add_recent_path(config, test_path.clone())
            .unwrap();
        let config = config_manager
            .add_recent_path(config, test_path.clone())
            .unwrap();

        assert_eq!(config.recent_paths.len(), 1);
    }

    #[test]
    fn test_dotfiles_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let dotfiles_repo = "https://github.com/user/dotfiles".to_string();
        let updated_config = config_manager
            .set_dotfiles_repo(config, Some(dotfiles_repo.clone()))
            .unwrap();

        assert_eq!(updated_config.dotfiles_repo, Some(dotfiles_repo));
    }

    #[test]
    fn test_features_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let feature = "ghcr.io/devcontainers/features/github-cli:1".to_string();
        let value = "latest".to_string();
        let updated_config = config_manager
            .add_feature(config, feature.clone(), value.clone())
            .unwrap();

        assert_eq!(
            updated_config.additional_features.get(&feature),
            Some(&value)
        );

        let cleared_config = config_manager.clear_features(updated_config).unwrap();
        assert!(cleared_config.additional_features.is_empty());
    }

    #[test]
    fn test_env_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let env = "CODESPACES".to_string();
        let value = "true".to_string();
        let updated_config = config_manager
            .add_env(
                config,
                env.clone(),
                value.clone(),
                Some(DevContainerContext::Exec),
            )
            .unwrap();

        assert_eq!(
            updated_config.env.first(),
            Some(&AppConfigEnv {
                name: env,
                value,
                context: DevContainerContext::Exec,
            })
        );

        let removed_config = config_manager
            .remove_env(updated_config.clone(), 0)
            .unwrap();
        assert!(removed_config.env.is_empty());

        let env = "CODESPACES".to_string();
        let value = "true".to_string();
        let updated_config2 = config_manager
            .add_env(
                updated_config,
                env.clone(),
                value.clone(),
                Some(DevContainerContext::Exec),
            )
            .unwrap();

        assert_eq!(
            updated_config2.env.first(),
            Some(&AppConfigEnv {
                name: env,
                value,
                context: DevContainerContext::Exec,
            })
        );

        let cleared_config = config_manager.clear_env(updated_config2).unwrap();
        assert!(cleared_config.env.is_empty());

        let removed2_config = config_manager.remove_env(cleared_config.clone(), 0);
        assert!(removed2_config.is_err());
    }

    #[test]
    fn test_list_env_by_context() {
        let mut config = AppConfig::default();

        // Add environment variables with different contexts
        config.env.push(AppConfigEnv {
            name: "GLOBAL_VAR".to_string(),
            value: "global_value".to_string(),
            context: DevContainerContext::All,
        });

        config.env.push(AppConfigEnv {
            name: "UP_VAR".to_string(),
            value: "up_value".to_string(),
            context: DevContainerContext::Up,
        });

        config.env.push(AppConfigEnv {
            name: "EXEC_VAR".to_string(),
            value: "exec_value".to_string(),
            context: DevContainerContext::Exec,
        });

        config.env.push(AppConfigEnv {
            name: "ANOTHER_GLOBAL".to_string(),
            value: "another_global_value".to_string(),
            context: DevContainerContext::All,
        });

        // Test filtering by Up context - should return All + Up
        let up_env = config.list_env_by_context(DevContainerContext::Up);
        assert_eq!(up_env.len(), 3);
        assert!(up_env.iter().any(|env| env.name == "GLOBAL_VAR"));
        assert!(up_env.iter().any(|env| env.name == "UP_VAR"));
        assert!(up_env.iter().any(|env| env.name == "ANOTHER_GLOBAL"));
        assert!(!up_env.iter().any(|env| env.name == "EXEC_VAR"));

        // Test filtering by Exec context - should return All + Exec
        let exec_env = config.list_env_by_context(DevContainerContext::Exec);
        assert_eq!(exec_env.len(), 3);
        assert!(exec_env.iter().any(|env| env.name == "GLOBAL_VAR"));
        assert!(exec_env.iter().any(|env| env.name == "EXEC_VAR"));
        assert!(exec_env.iter().any(|env| env.name == "ANOTHER_GLOBAL"));
        assert!(!exec_env.iter().any(|env| env.name == "UP_VAR"));

        // Test filtering by All context - should return only All
        let all_env = config.list_env_by_context(DevContainerContext::All);
        assert_eq!(all_env.len(), 2);
        assert!(all_env.iter().any(|env| env.name == "GLOBAL_VAR"));
        assert!(all_env.iter().any(|env| env.name == "ANOTHER_GLOBAL"));
        assert!(!all_env.iter().any(|env| env.name == "UP_VAR"));
        assert!(!all_env.iter().any(|env| env.name == "EXEC_VAR"));
    }

    #[test]
    fn test_devcontainer_context_from_string() {
        assert_eq!(
            "all".parse::<DevContainerContext>().unwrap(),
            DevContainerContext::All
        );
        assert_eq!(
            "up".parse::<DevContainerContext>().unwrap(),
            DevContainerContext::Up
        );
        assert_eq!(
            "exec".parse::<DevContainerContext>().unwrap(),
            DevContainerContext::Exec
        );

        assert!("invalid".parse::<DevContainerContext>().is_err());
    }

    #[test]
    fn test_devcontainer_context_default() {
        let default_context = DevContainerContext::default();
        assert_eq!(default_context, DevContainerContext::All);
    }

    #[test]
    fn test_list_env_by_context_edge_cases() {
        let mut config = AppConfig::default();

        // Test with empty env list
        let empty_env = config.list_env_by_context(DevContainerContext::Up);
        assert!(empty_env.is_empty());

        // Add multiple env vars with same context
        config.env.push(AppConfigEnv {
            name: "VAR1".to_string(),
            value: "value1".to_string(),
            context: DevContainerContext::Up,
        });

        config.env.push(AppConfigEnv {
            name: "VAR2".to_string(),
            value: "value2".to_string(),
            context: DevContainerContext::Up,
        });

        let up_env = config.list_env_by_context(DevContainerContext::Up);
        assert_eq!(up_env.len(), 2);

        // Test that All context items appear in all queries
        config.env.push(AppConfigEnv {
            name: "GLOBAL_VAR".to_string(),
            value: "global_value".to_string(),
            context: DevContainerContext::All,
        });

        let up_env_with_global = config.list_env_by_context(DevContainerContext::Up);
        assert_eq!(up_env_with_global.len(), 3);

        let exec_env = config.list_env_by_context(DevContainerContext::Exec);
        assert_eq!(exec_env.len(), 1); // Only the global one

        let all_env = config.list_env_by_context(DevContainerContext::All);
        assert_eq!(all_env.len(), 1); // Only items specifically marked as All
    }

    #[test]
    fn test_env_configuration_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        // Test adding env with None context (should default to All)
        let updated_config = config_manager
            .add_env(
                config,
                "TEST_VAR".to_string(),
                "test_value".to_string(),
                None,
            )
            .unwrap();

        assert_eq!(updated_config.env.len(), 1);
        assert_eq!(updated_config.env[0].context, DevContainerContext::All);

        // Test removing env with invalid index
        let result = config_manager.remove_env(updated_config.clone(), 999);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Index out of bounds")
        );

        // Test removing env with valid index
        let removed_config = config_manager.remove_env(updated_config, 0).unwrap();
        assert!(removed_config.env.is_empty());
    }

    #[test]
    fn test_config_serialization_with_env() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
        let mut config = AppConfig::default();

        // Add environment variables
        config.env.push(AppConfigEnv {
            name: "EDITOR".to_string(),
            value: "vim".to_string(),
            context: DevContainerContext::All,
        });

        config.env.push(AppConfigEnv {
            name: "DEBUG".to_string(),
            value: "true".to_string(),
            context: DevContainerContext::Exec,
        });

        // Save and reload
        config_manager.save_config(&config).unwrap();
        if let Ok(reloaded_config) = config_manager.load_config() {
            assert_eq!(reloaded_config.env.len(), 2);
            assert_eq!(reloaded_config.env[0].name, "EDITOR");
            assert_eq!(reloaded_config.env[0].context, DevContainerContext::All);
            assert_eq!(reloaded_config.env[1].name, "DEBUG");
            assert_eq!(reloaded_config.env[1].context, DevContainerContext::Exec);
            assert_eq!(reloaded_config.recent_paths.len(), 0);
        } else {
            panic!(
                "Failed to reload config from \"{}\"",
                &temp_dir.path().display()
            );
        }
    }
}
