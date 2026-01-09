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

//! # Container Drivers
//!
//! This module provides the core functionality for building and managing
//! development containers.
//!
//! ## Overview
//!
//! The driver module handles:
//! - Processing and downloading devcontainer features from registries
//! - Building container images with Dockerfiles
//! - Starting and managing container instances
//!
//! ## Submodules
//!
//! - [`container`] - Container lifecycle management (build, start, stop)
//!
//! ## Feature Processing
//!
//! Features can be sourced from:
//! - **Registry** - Downloaded from OCI-compliant registries like ghcr.io
//! - **Local** - Loaded from the local filesystem (not yet implemented)

use std::fs::{self, File};

use anyhow::bail;
use serde_json::Value;
use tempfile::TempDir;

use crate::devcontainer::Feature;

pub mod container;
pub mod runtime;

struct FeatureProcessResult {
    pub feature: Feature,
    #[allow(dead_code)]
    pub options: serde_json::Value,
    pub relative_path: String,
}

/// Processes a list of features, downloading and extracting them as needed.
///
/// This function iterates through all features and processes each one,
/// returning a list of tuples containing the feature reference and its
/// relative path in the temporary directory.
///
/// # Arguments
///
/// * `features` - Slice of features to process
/// * `directory` - Temporary directory where features will be stored
///
/// # Returns
///
/// A vector of tuples containing:
/// - Reference to the feature
/// - Relative path to the extracted feature files
///
/// # Errors
///
/// Returns an error if any feature fails to download or extract.
fn process_features<'a>(
    features: &'a [Feature],
    directory: &'a TempDir,
) -> anyhow::Result<Vec<FeatureProcessResult>> {
    let mut result: Vec<FeatureProcessResult> = vec![];
    for feature in features {
        let feature_result = process_feature(feature, directory)?;
        result.push(feature_result);
    }
    Ok(result)
}

fn process_feature<'a>(
    feature: &'a Feature,
    directory: &'a TempDir,
) -> anyhow::Result<FeatureProcessResult> {
    let relative_path = match &feature.source {
        crate::devcontainer::FeatureSource::Registry {
            registry_type,
            registry,
        } => download_feature(registry_type, registry, directory),
        crate::devcontainer::FeatureSource::Local { path: _ } => {
            todo!("Local feature source not yet implemented")
        }
    }?;

    // Read devcontainer-feature.json if it exists to get default options
    let feature_json_path = directory
        .path()
        .join(&relative_path)
        .join("devcontainer-feature.json");

    let mut feature_options = serde_json::json!({});
    if feature_json_path.exists() {
        let feature_json_content = fs::read_to_string(&feature_json_path)?;
        let feature_json: Value = serde_json::from_str(&feature_json_content)?;
        if let Some(Value::Object(default_map)) = feature_json.get("options") {
            default_map.iter().for_each(|(key, value)| {
                feature_options
                    .as_object_mut()
                    .unwrap()
                    .insert(key.clone(), value["default"].clone());
            });
        }
    }

    // Override default options with user-specified options
    if feature.options.is_object() {
        for (key, value) in feature.options.as_object().unwrap() {
            if feature_options.get(key).is_some() {
                feature_options[key] = value.clone();
            }
        }
    }

    // Creating env variable file devcontainer-features.env which can be sourced during feature installation
    let env_file_path = directory
        .path()
        .join(&relative_path)
        .join("devcontainer-features.env");
    let mut env_file = File::create(&env_file_path)?;
    for (key, value) in feature_options.as_object().unwrap() {
        use std::io::Write;
        writeln!(
            env_file,
            "export {}={}",
            key.to_uppercase(),
            value.as_str().unwrap_or("")
        )?;
    }

    Ok(FeatureProcessResult {
        feature: feature.clone(),
        options: feature_options,
        relative_path,
    })
}

fn download_feature<'a>(
    _registry_type: &'a crate::devcontainer::FeatureRegistryType,
    registry: &'a crate::devcontainer::FeatureRegistry,
    directory: &'a TempDir,
) -> anyhow::Result<String> {
    let feature_directory = directory.path().join(&registry.name);
    fs::create_dir_all(&feature_directory)?;

    let token_url = format!(
        "https://{}/token?scope=repository:{}/{}:pull",
        "ghcr.io", registry.owner, registry.repository
    );

    let response = reqwest::blocking::get(&token_url)?;
    if !response.status().is_success() {
        bail!("Failed to download feature: {}", registry.name);
    }
    let json: serde_json::Value = response.json()?;
    let token = json["token"].as_str().ok_or_else(|| {
        anyhow::anyhow!("Token not found in response for feature: {}", registry.name)
    })?;

    let manifest_url = format!(
        "https://{}/v2/{}/{}/{}/manifests/{}",
        "ghcr.io", registry.owner, registry.repository, registry.name, registry.version
    );

    let manifest_response = reqwest::blocking::Client::new()
        .get(&manifest_url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.oci.image.manifest.v1+json")
        .send()?;

    if !manifest_response.status().is_success() {
        bail!("Failed to download manifest for feature: {}", registry.name);
    }
    let manifest_json: serde_json::Value = manifest_response.json()?;
    let manifest_str = serde_json::to_string(&manifest_json)?;
    let reader = std::io::Cursor::new(manifest_str);
    let _manifest = oci_spec::image::ImageManifest::from_reader(reader)?;
    let layer = _manifest.layers().first().ok_or_else(|| {
        anyhow::anyhow!("No layers found in manifest for feature: {}", registry.name)
    })?;

    let layer_url = format!(
        "https://{}/v2/{}/{}/{}/blobs/{}",
        "ghcr.io",
        registry.owner,
        registry.repository,
        registry.name,
        layer.digest()
    );
    let layer_response = reqwest::blocking::Client::new()
        .get(&layer_url)
        .bearer_auth(token)
        .send()?;

    if !layer_response.status().is_success() {
        bail!("Failed to download layer for feature: {}", registry.name);
    }
    let _layer_bytes = layer_response.bytes()?;

    let feature_file = format!("feature-{}.tar.gz", registry.name);
    let feature_file_path = &feature_directory.join(&feature_file);
    fs::write(feature_file_path, &_layer_bytes)?;

    let feature_archive = File::open(feature_file_path)?;
    let mut archive = tar::Archive::new(flate2::write::GzDecoder::new(feature_archive));
    let extract_path = &feature_directory.join("extract");
    fs::create_dir_all(extract_path)?;
    archive.unpack(extract_path)?;
    let extracted_path = extract_path
        .to_str()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to convert path to str for feature: {}",
                registry.name
            )
        })?
        .to_string();

    let relative_path = extracted_path
        .strip_prefix(directory.path().to_str().unwrap())
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get relative path for feature: {}", registry.name)
        })?
        .trim_matches('/')
        .to_string();

    Ok(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_download_feature() {
        let registry = crate::devcontainer::FeatureRegistry {
            owner: "devcontainers".to_string(),
            repository: "features".to_string(),
            name: "node".to_string(),
            version: "1.0.0".to_string(),
        };
        let temp_dir = tempdir().unwrap();
        let result = download_feature(
            &crate::devcontainer::FeatureRegistryType::Ghcr,
            &registry,
            &temp_dir,
        );
        assert!(result.is_ok());
        let relative_path = result.unwrap();
        let feature_path = temp_dir.path().join(&relative_path);
        assert!(feature_path.exists());
    }

    #[test]
    fn test_process_feature() {
        let feature = Feature {
            source: crate::devcontainer::FeatureSource::Registry {
                registry_type: crate::devcontainer::FeatureRegistryType::Ghcr,
                registry: crate::devcontainer::FeatureRegistry {
                    owner: "devcontainers".to_string(),
                    repository: "features".to_string(),
                    name: "node".to_string(),
                    version: "1.0.0".to_string(),
                },
            },
            options: serde_json::json!({
                "version": "16"
            }),
        };
        let temp_dir = tempdir().unwrap();
        let result = process_feature(&feature, &temp_dir);
        assert!(result.is_ok());
        let feature_result = result.unwrap();
        assert_eq!(feature_result.options["version"], "16");
    }

    #[test]
    fn test_process_feature_default() {
        let feature = Feature {
            source: crate::devcontainer::FeatureSource::Registry {
                registry_type: crate::devcontainer::FeatureRegistryType::Ghcr,
                registry: crate::devcontainer::FeatureRegistry {
                    owner: "devcontainers".to_string(),
                    repository: "features".to_string(),
                    name: "node".to_string(),
                    version: "1.0.0".to_string(),
                },
            },
            options: serde_json::json!({}),
        };
        let temp_dir = tempdir().unwrap();
        let result = process_feature(&feature, &temp_dir);
        assert!(result.is_ok());
        let feature_result = result.unwrap();
        assert_eq!(feature_result.options["version"], "lts");
    }
}
