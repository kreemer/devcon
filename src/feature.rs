// MIT License
//
// Copyright (c) 2025 DevCon Contributors

//! # Devcontainer Features
//!
//! This module provides types and functionality for working with devcontainer features.
//!
//! ## Main Types
//!
//! - [`FeatureRef`] - A reference to a feature in devcontainer.json (URL + user options)
//! - [`FeatureMetadata`] - The full feature definition from devcontainer-feature.json
//! - [`FeatureSource`] - Defines where a feature comes from (registry or local)

use serde::de;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents the metadata from a devcontainer-feature.json file.
///
/// This is the full feature definition that describes what the feature does,
/// what options it accepts, and how it should be installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    /// Unique identifier for the feature (required)
    pub id: String,

    /// Version following semver specification (required)
    pub version: String,

    /// Display name of the feature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Description of the feature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// URL to documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,

    /// URL to the license
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,

    /// Keywords for searching
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,

    /// User-configurable options schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, FeatureOption>>,

    /// Array of feature IDs that should execute before this one
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installs_after: Option<Vec<String>>,

    /// Feature dependencies that must be satisfied
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<HashMap<String, serde_json::Value>>,

    /// Indicates the feature is deprecated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,

    /// Old IDs used for renaming this feature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_ids: Option<Vec<String>>,

    /// Docker capabilities to add
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_add: Option<Vec<String>>,

    /// Container security options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_opt: Option<Vec<String>>,

    /// Sets privileged mode for the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privileged: Option<bool>,

    /// Adds tiny init process to the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<bool>,

    /// Entrypoint script that fires at container startup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,

    /// Mounts for volumes or bind mounts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<FeatureMount>>,

    /// Container environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_env: Option<HashMap<String, String>>,

    /// Tool-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customizations: Option<HashMap<String, serde_json::Value>>,

    /// Command to run when creating the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_create_command: Option<LifecycleCommand>,

    /// Command to run when workspace content is updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_content_command: Option<LifecycleCommand>,

    /// Command to run after creating the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_create_command: Option<LifecycleCommand>,

    /// Command to run after starting the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_start_command: Option<LifecycleCommand>,

    /// Command to run when attaching to the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_attach_command: Option<LifecycleCommand>,
}

/// Represents a lifecycle command that can be a string, array, or object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LifecycleCommand {
    /// Single command as a string
    String(String),
    /// Multiple commands as an array
    Array(Vec<String>),
    /// Named commands as an object
    Object(HashMap<String, LifecycleCommandValue>),
}

/// Represents a value in a lifecycle command object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LifecycleCommandValue {
    String(String),
    Array(Vec<String>),
}

/// Mount configuration for volumes or bind mounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FeatureMount {
    /// String format mount
    String(String),
    /// Structured mount configuration
    Structured(StructuredMount),
}

/// Structured mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredMount {
    /// Type of mount (bind or volume)
    #[serde(rename = "type")]
    pub mount_type: MountType,

    /// Mount source (optional for volume mounts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Mount target (required)
    pub target: String,
}

/// Type of mount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountType {
    Bind,
    Volume,
}

/// Configuration option for a feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureOption {
    /// Type of the option (boolean or string)
    #[serde(rename = "type")]
    pub option_type: FeatureOptionType,

    /// Description displayed to the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Default value
    pub default: serde_json::Value,

    /// Allowed values (user cannot provide custom values)
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<String>>,

    /// Suggested values (user can provide custom values)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposals: Option<Vec<String>>,
}

/// Type of feature option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureOptionType {
    Boolean,
    String,
}
