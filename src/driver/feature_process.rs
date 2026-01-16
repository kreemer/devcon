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

use std::{
    fs::{self, File},
    path::PathBuf,
};

use anyhow::{Ok, bail};
use dircpy::copy_dir;
use indicatif::ProgressBar;
use oci_spec::image::MediaType::{ImageLayer, ImageLayerGzip, ImageLayerNonDistributableGzip};
use serde_json::Value;
use tempfile::TempDir;
use tracing::info;

use crate::devcontainer::{
    FeatureRef, FeatureRegistry, FeatureRegistryType,
    FeatureSource::{Local, Registry},
};
use crate::feature::Feature;

pub struct FeatureProcessResult {
    pub feature_ref: FeatureRef,
    pub feature: Feature,
    pub path: PathBuf,
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
pub fn process_features<'a>(
    features: &'a [FeatureRef],
) -> anyhow::Result<Vec<FeatureProcessResult>> {
    println!("Processing features..");
    let bar = ProgressBar::new(u64::try_from(features.len())?);
    let mut result: Vec<FeatureProcessResult> = vec![];
    for feature_ref in features {
        match &feature_ref.source {
            Registry { registry, .. } => {
                bar.println(format!("Processing feature {}", registry.name))
            }
            Local { path } => bar.println(format!(
                "Processing feature {}",
                path.canonicalize()?
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Could not get basename of directory"))?
                    .to_string_lossy()
                    .to_string()
            )),
        }
        let feature_result = process_feature(feature_ref)?;
        result.push(feature_result);
        bar.inc(1);
    }
    bar.finish();
    Ok(result)
}

fn process_feature<'a>(feature_ref: &'a FeatureRef) -> anyhow::Result<FeatureProcessResult> {
    let relative_path = match &feature_ref.source {
        Registry { registry } => download_feature(registry),
        Local { path } => local_feature(path),
    }?;

    // Read devcontainer-feature.json if it exists to parse the Feature metadata
    let feature_json_path = relative_path.join("devcontainer-feature.json");

    if !feature_json_path.exists() {
        bail!(
            "Feature definition file not found: {}",
            feature_json_path.display()
        );
    }

    let feature_json_content = fs::read_to_string(&feature_json_path)?;
    let parsed_feature: Feature = serde_json::from_str(&feature_json_content)?;

    // Create env variable file with merged options (defaults + user overrides)
    let mut feature_options = serde_json::json!({});

    // Start with default values from feature definition
    if let Some(ref options_map) = parsed_feature.options {
        for (key, option) in options_map {
            feature_options
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), option.default.clone());
        }
    }

    // Override with user-specified options from feature_ref
    if let Some(user_opts) = feature_ref.options.as_object() {
        for (key, value) in user_opts {
            feature_options
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
        }
    }
    // TODO: Move this to the container after copying this to the build directory
    // Create env variable file for feature installation
    let env_file_path = relative_path.join("devcontainer-features.env");
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
        feature_ref: feature_ref.clone(),
        feature: parsed_feature,
        path: relative_path,
    })
}

/// Get the cache directory for devcontainer features
fn get_feature_cache_dir() -> anyhow::Result<std::path::PathBuf> {
    let cache_dir =
        dirs::cache_dir().ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?;
    let devcon_cache = cache_dir.join("devcon").join("features");
    fs::create_dir_all(&devcon_cache)?;
    Ok(devcon_cache)
}

/// Get the versioned cache path for a specific feature
fn get_cached_feature_path(registry: &FeatureRegistry) -> anyhow::Result<std::path::PathBuf> {
    let cache_dir = get_feature_cache_dir()?;
    // Create path: cache/owner/repository/name/version
    let feature_cache = cache_dir
        .join(&registry.owner)
        .join(&registry.repository)
        .join(&registry.name)
        .join(&registry.version);
    Ok(feature_cache)
}

/// Get local feature path
fn local_feature(path: &PathBuf) -> anyhow::Result<PathBuf> {
    path.canonicalize().map_err(|e| anyhow::anyhow!(e))
}

/// Download a feature from registry to cache, or use cached version if available
fn download_feature<'a>(registry: &'a FeatureRegistry) -> anyhow::Result<PathBuf> {
    let cached_feature_path = get_cached_feature_path(registry)?;

    // Check if feature is already cached
    if !cached_feature_path.exists()
        || !cached_feature_path
            .join("devcontainer-features.json")
            .exists()
    {
        info!(
            "Downloading feature: {} (version {})",
            registry.name, registry.version
        );
        download_and_cache_feature(registry, &cached_feature_path)?;
    } else {
        info!(
            "Using cached feature: {} (version {})",
            registry.name, registry.version
        );
    }

    Ok(cached_feature_path)
}

/// Download and extract a feature to the cache directory
fn download_and_cache_feature(
    registry: &FeatureRegistry,
    cache_path: &std::path::Path,
) -> anyhow::Result<()> {
    let temp_directory = TempDir::new()?;

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
    let manifest = oci_spec::image::ImageManifest::from_reader(reader)?;
    let layer = manifest.layers().first().ok_or_else(|| {
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
    let layer_bytes = layer_response.bytes()?;

    let extract_path = match layer.media_type() {
        oci_spec::image::MediaType::Other(str) => match str.as_str() {
            "application/vnd.devcontainers.layer.v1+tar"
            | "application/vnd.oci.image.layer.v1.tar" => {
                let temp_file = temp_directory.path().join("feature.tar");
                fs::write(&temp_file, &layer_bytes)?;

                let feature_archive = File::open(&temp_file)?;
                let mut archive = tar::Archive::new(feature_archive);
                let extract_path = temp_directory.path().join("extract");
                fs::create_dir_all(&extract_path)?;
                archive.unpack(&extract_path).unwrap();

                extract_path
            }
            "application/vnd.devcontainers.layer.v1+tar+gzip"
            | "application/vnd.oci.image.layer.v1.tar+gzip" => {
                let temp_file = temp_directory.path().join("feature.tar.gz");
                fs::write(&temp_file, &layer_bytes)?;

                let feature_archive = File::open(&temp_file)?;
                let decompressor = flate2::read::GzDecoder::new(feature_archive);
                let mut archive = tar::Archive::new(decompressor);
                let extract_path = temp_directory.path().join("extract");
                fs::create_dir_all(&extract_path)?;
                archive.unpack(&extract_path).unwrap();

                extract_path
            }
            _ => {
                bail!(
                    "Unsupported layer media type for feature: {}, media type: {}",
                    registry.name,
                    str
                );
            }
        },

        _ => {
            bail!(
                "Unsupported layer media type for feature: {}",
                registry.name
            );
        }
    };

    // Move extracted feature to cache path
    fs::create_dir_all(cache_path)?;
    copy_dir(extract_path, cache_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::devcontainer::FeatureSource;

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_download_feature() {
        let registry = FeatureRegistry {
            owner: "devcontainers".to_string(),
            repository: "features".to_string(),
            name: "node".to_string(),
            version: "1.0.0".to_string(),
            registry_type: FeatureRegistryType::Ghcr,
        };
        let temp_dir = tempdir().unwrap();
        let result = download_feature(&registry);
        assert!(
            result.is_ok(),
            "Failed to download feature: {:?}",
            result.err()
        );
        let relative_path = result.unwrap();
        let feature_path = temp_dir.path().join(&relative_path);
        assert!(feature_path.exists());
    }

    #[test]
    fn test_process_feature() {
        let feature_ref = FeatureRef::new(FeatureSource::Registry {
            registry: FeatureRegistry {
                owner: "devcontainers".to_string(),
                repository: "features".to_string(),
                name: "node".to_string(),
                version: "1.0.0".to_string(),
                registry_type: FeatureRegistryType::Ghcr,
            },
        });
        let result = process_feature(&feature_ref);
        assert!(
            result.is_ok(),
            "Failed to download feature: {:?}",
            result.err()
        );
        let feature_result = result.unwrap();
        let feature = feature_result.feature;
        // Check that default option exists
        if let Some(ref options) = feature.options {
            assert!(options.contains_key("version"));
        }
    }

    #[test]
    fn test_process_feature_default() {
        let feature_ref = FeatureRef::new(FeatureSource::Registry {
            registry: FeatureRegistry {
                owner: "devcontainers".to_string(),
                repository: "features".to_string(),
                name: "node".to_string(),
                version: "1.0.0".to_string(),
                registry_type: FeatureRegistryType::Ghcr,
            },
        });
        let result = process_feature(&feature_ref);
        assert!(
            result.is_ok(),
            "Failed to download feature: {:?}",
            result.err()
        );
    }
}
