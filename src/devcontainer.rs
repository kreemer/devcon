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

//! # Devcontainer Configuration
//!
//! This module provides types and functionality for parsing and working with
//! devcontainer.json configuration files.
//!
//! ## Overview
//!
//! The devcontainer specification defines how to configure development containers
//! with specific tools, runtime environments, and features. This module implements
//! parsing and deserialization of these configurations.
//!
//! ## Main Types
//!
//! - [`Devcontainer`] - The main configuration structure
//! - [`Feature`] - Represents a devcontainer feature with its source and options
//! - [`FeatureSource`] - Defines where a feature comes from (registry or local)
//! - [`FeatureRegistry`] - Registry-specific feature metadata
//!
//! ## Examples
//!
//! ```no_run
//! use std::path::PathBuf;
//! use devcon::devcontainer::Devcontainer;
//!
//! let config = Devcontainer::try_from(PathBuf::from("/path/to/project"))?;
//! println!("Container name: {}", config.get_computed_name());
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::fs;
use std::path::PathBuf;

use anyhow::bail;
use serde::Deserialize;
use serde::de;

/// Represents a devcontainer.json configuration.
///
/// This structure contains all the necessary information to build and run
/// a development container, including the base image, features, and user settings.
///
/// # Fields
///
/// * `name` - Optional name for the container (defaults to directory name if not set)
/// * `image` - The Docker base image to use
/// * `features` - List of features to install in the container
/// * `remote_user` - The user to use when connecting to the container
#[derive(Debug)]
pub struct Devcontainer {
    pub name: Option<String>,
    pub image: String,
    pub features: Vec<Feature>,
    #[allow(dead_code)]
    pub remote_user: Option<String>,
}

impl<'de> Deserialize<'de> for Devcontainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DevcontainerHelper {
            name: Option<String>,
            image: String,
            #[serde(default)]
            #[serde(deserialize_with = "deserialize_features_map")]
            features: Vec<(String, serde_json::Value)>,
            #[serde(rename = "remoteUser")]
            remote_user: Option<String>,
        }

        let helper = DevcontainerHelper::deserialize(deserializer)?;

        let features: Result<Vec<Feature>, D::Error> = helper
            .features
            .into_iter()
            .map(|(url, options)| parse_feature(&url, options))
            .collect();

        Ok(Devcontainer {
            name: helper.name,
            image: helper.image,
            features: features?,
            remote_user: helper.remote_user,
        })
    }
}

fn deserialize_features_map<'de, D>(
    deserializer: D,
) -> Result<Vec<(String, serde_json::Value)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FeaturesVisitor;

    impl<'de> serde::de::Visitor<'de> for FeaturesVisitor {
        type Value = Vec<(String, serde_json::Value)>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map of features")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut features = Vec::new();
            while let Some((key, value)) = map.next_entry()? {
                features.push((key, value));
            }
            Ok(features)
        }
    }

    deserializer.deserialize_map(FeaturesVisitor)
}

impl Devcontainer {
    pub fn get_computed_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| "default".to_string())
    }

    /// Merges additional features from configuration into this devcontainer.
    ///
    /// This method adds features from the config that aren't already present
    /// in the devcontainer.json. Existing features take precedence.
    ///
    /// # Arguments
    ///
    /// * `additional_features` - HashMap of feature URLs to their options
    ///
    /// # Errors
    ///
    /// Returns an error if any additional feature cannot be parsed.
    pub fn merge_additional_features(
        &mut self,
        additional_features: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<()> {
        use std::collections::HashSet;

        // Get set of existing feature URLs
        let existing_urls: HashSet<String> = self
            .features
            .iter()
            .filter_map(|f| match &f.source {
                FeatureSource::Registry { registry, .. } => Some(format!(
                    "ghcr.io/{}/{}/{}:{}",
                    registry.owner, registry.repository, registry.name, registry.version
                )),
                FeatureSource::Local { path } => Some(path.to_string_lossy().to_string()),
            })
            .collect();

        // Add features that don't already exist
        for (url, options) in additional_features {
            if !existing_urls.contains(url) {
                let feature = parse_feature::<serde::de::value::Error>(url, options.clone())
                    .map_err(|e| anyhow::anyhow!("Failed to parse additional feature: {}", e))?;
                self.features.push(feature);
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Feature {
    pub source: FeatureSource,
    #[allow(dead_code)]
    pub options: serde_json::Value,
}

#[derive(Debug)]
pub enum FeatureSource {
    Registry {
        registry_type: FeatureRegistryType,
        registry: FeatureRegistry,
    },
    Local {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub struct FeatureRegistry {
    pub owner: String,
    pub repository: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug)]
pub enum FeatureRegistryType {
    Ghcr,
}

fn parse_feature<E: de::Error>(url: &str, options: serde_json::Value) -> Result<Feature, E> {
    if !url.starts_with("ghcr.io") && url.contains(":") {
        return Err(de::Error::custom("Only ghcr.io features are supported"));
    }

    if url.starts_with("ghcr.io") {
        parse_registry_feature(url, options)
    } else {
        parse_local_feature(url, options)
    }
}

fn parse_local_feature<E: de::Error>(url: &str, options: serde_json::Value) -> Result<Feature, E> {
    let path = PathBuf::from(url);
    Ok(Feature {
        source: FeatureSource::Local { path },
        options,
    })
}

fn parse_registry_feature<E: de::Error>(
    url: &str,
    options: serde_json::Value,
) -> Result<Feature, E> {
    let owner = url
        .split("/")
        .nth(1)
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing owner information"))?;
    let repository = url
        .split("/")
        .nth(2)
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing repository information"))?;
    let name = url
        .split("/")
        .nth(3)
        .and_then(|s| s.split(":").next())
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing name information"))?;

    let version = url
        .split("/")
        .nth(3)
        .and_then(|s| s.split(":").nth(1))
        .unwrap_or("latest");

    Ok(Feature {
        source: FeatureSource::Registry {
            registry_type: FeatureRegistryType::Ghcr,
            registry: FeatureRegistry {
                owner: owner.to_string(),
                repository: repository.to_string(),
                name: name.to_string(),
                version: version.to_string(),
            },
        },
        options,
    })
}

impl TryFrom<PathBuf> for Devcontainer {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let final_path = path.join(".devcontainer").join("devcontainer.json");
        if fs::exists(&final_path).is_err() {
            bail!(
                "Devcontainer definition not found in {}",
                &final_path.to_string_lossy()
            )
        }

        let file_result = fs::read_to_string(&final_path);

        if file_result.is_err() {
            bail!(
                "Devcontainer definition cannot be read {}",
                &final_path.to_string_lossy()
            )
        }

        let result = Self::try_from(file_result.unwrap());
        if result.is_err() {
            bail!("Devcontainer content could not be parsed")
        }

        // Fix name of container if not present
        let mut result = result?;
        if result.name.is_none() {
            let name = fs::canonicalize(&path)?
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid path for devcontainer"))?
                .to_string_lossy()
                .to_string();
            result.name = Some(name);
        }

        Ok(result)
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
    fn test_feature() {
        let feature = Feature {
            source: FeatureSource::Registry {
                registry_type: FeatureRegistryType::Ghcr,
                registry: FeatureRegistry {
                    owner: "devcontainers".to_string(),
                    repository: "features".to_string(),
                    name: "github-cli".to_string(),
                    version: "1".to_string(),
                },
            },
            options: serde_json::Value::Null,
        };

        assert!(feature.options.is_null());
        match feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_feature_parsing() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/github-cli:1": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 1);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_multiple_feature_parsing() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/github-cli:1": {},
               "ghcr.io/devcontainers/features/node:2": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 2);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
        let feature = &devcontainer.features[1];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("node", registry.name);
                assert_eq!("2", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_local_feature() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "./devfeatures/myfeature": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 1);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Local { path } => {
                assert_eq!(PathBuf::from("./devfeatures/myfeature"), *path);
            }
            _ => assert!(false, "Feature source should be Local"),
        }
    }

    #[test]
    fn test_mixed_features() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/github-cli:1": {},
               "./local-feature": {},
               "ghcr.io/devcontainers/features/node:2": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.features.len(), 3);

        // First should be registry
        match &devcontainer.features[0].source {
            FeatureSource::Registry { registry, .. } => {
                assert_eq!("github-cli", registry.name);
            }
            _ => panic!("Expected Registry feature"),
        }

        // Second should be local
        match &devcontainer.features[1].source {
            FeatureSource::Local { path } => {
                assert_eq!(PathBuf::from("./local-feature"), *path);
            }
            _ => panic!("Expected Local feature"),
        }

        // Third should be registry
        match &devcontainer.features[2].source {
            FeatureSource::Registry { registry, .. } => {
                assert_eq!("node", registry.name);
            }
            _ => panic!("Expected Registry feature"),
        }
    }

    #[test]
    fn test_feature_with_options() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/node:2": {
                   "version": "18",
                   "installYarnUsingApt": true
               }
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.features.len(), 1);
        let feature = &devcontainer.features[0];

        assert!(feature.options.is_object());
        assert_eq!(
            feature.options.get("version").and_then(|v| v.as_str()),
            Some("18")
        );
        assert_eq!(
            feature
                .options
                .get("installYarnUsingApt")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_feature_without_version() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/docker-in-docker": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Registry { registry, .. } => {
                assert_eq!("latest", registry.version);
            }
            _ => panic!("Expected Registry feature"),
        }
    }

    #[test]
    fn test_empty_features() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {}
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.features.len(), 0);
    }

    #[test]
    fn test_no_features_field() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04"
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.features.len(), 0);
    }

    #[test]
    fn test_missing_name_field() {
        let feature_json = r#"
        {
            "image": "ubuntu:20.04",
            "features": {}
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name, None);
        assert_eq!(devcontainer.get_computed_name(), "default");
    }

    #[test]
    fn test_with_remote_user() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "remoteUser": "vscode"
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.remote_user.as_deref(), Some("vscode"));
    }

    #[test]
    fn test_get_computed_name_with_name() {
        let feature_json = r#"
        {
            "name": "my-project",
            "image": "ubuntu:20.04"
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.get_computed_name(), "my-project");
    }

    #[test]
    fn test_invalid_feature_url_missing_owner() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/invalid:1": {}
            }
        }
        "#;

        let result: Result<Devcontainer, _> = serde_json::from_str(feature_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_feature_url_missing_name() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features": {}
            }
        }
        "#;

        let result: Result<Devcontainer, _> = serde_json::from_str(feature_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_image_field() {
        let feature_json = r#"
        {
            "name": "test",
            "features": {}
        }
        "#;

        let result: Result<Devcontainer, _> = serde_json::from_str(feature_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_string() {
        let content = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04"
        }
        "#;

        let devcontainer = Devcontainer::try_from(content.to_string()).unwrap();
        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.image, "ubuntu:20.04");
    }

    #[test]
    fn test_complex_feature_parsing() {
        let feature_json = r#"
        {
            "name": "complex-test",
            "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
            "features": {
                "ghcr.io/devcontainers/features/common-utils:2": {
                    "installZsh": true,
                    "installOhMyZsh": true,
                    "username": "vscode"
                },
                "ghcr.io/devcontainers/features/git:1": {
                    "version": "latest",
                    "ppa": true
                },
                "./local-features/custom-tool": {
                    "enabled": true
                }
            },
            "remoteUser": "vscode"
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("complex-test"));
        assert_eq!(
            devcontainer.image,
            "mcr.microsoft.com/devcontainers/base:ubuntu"
        );
        assert_eq!(devcontainer.features.len(), 3);
        assert_eq!(devcontainer.remote_user.as_deref(), Some("vscode"));

        // Verify first feature
        match &devcontainer.features[0].source {
            FeatureSource::Registry { registry, .. } => {
                assert_eq!("common-utils", registry.name);
                assert_eq!("2", registry.version);
            }
            _ => panic!("Expected Registry feature"),
        }

        // Verify second feature
        match &devcontainer.features[1].source {
            FeatureSource::Registry { registry, .. } => {
                assert_eq!("git", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => panic!("Expected Registry feature"),
        }

        // Verify third feature is local
        match &devcontainer.features[2].source {
            FeatureSource::Local { .. } => {}
            _ => panic!("Expected Local feature"),
        }
    }
}
