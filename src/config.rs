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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub recent_paths: Vec<PathBuf>,
    pub dotfiles_repo: Option<String>,
    pub additional_features: std::collections::HashMap<String, String>,
}

pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("devcon");
        let config_path = xdg_dirs.find_config_file("config.yaml").unwrap_or_else(|| {
            xdg_dirs
                .place_config_file("config.yaml")
                .expect("Cannot create config directory")
        });

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_config_creation() {
        let temp_dir = TempDir::new().unwrap();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let config_manager = ConfigManager::new().unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        assert!(config.recent_paths.is_empty());
    }

    #[test]
    fn test_add_recent_path() {
        let temp_dir = TempDir::new().unwrap();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let config_manager = ConfigManager::new().unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let test_path = temp_dir.path().to_path_buf();
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
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let config_manager = ConfigManager::new().unwrap();
        let config = config_manager.load_or_create_config().unwrap();

        let test_path = temp_dir.path().to_path_buf();
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
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let config_manager = ConfigManager::new().unwrap();
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
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let config_manager = ConfigManager::new().unwrap();
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
}
