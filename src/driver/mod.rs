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
use tempfile::TempDir;

use crate::devcontainer::Feature;

pub mod container;

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
) -> anyhow::Result<Vec<(&'a Feature, String)>> {
    let mut result: Vec<(&Feature, String)> = vec![];
    for feature in features {
        let relative_path = process_feature(feature, directory)?;
        result.push((feature, relative_path));
    }
    Ok(result)
}

fn process_feature<'a>(feature: &'a Feature, directory: &'a TempDir) -> anyhow::Result<String> {
    match &feature.source {
        crate::devcontainer::FeatureSource::Registry {
            registry_type,
            registry,
        } => download_feature(registry_type, registry, directory),
        crate::devcontainer::FeatureSource::Local { path: _ } => {
            todo!("Local feature source not yet implemented")
        }
    }
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
