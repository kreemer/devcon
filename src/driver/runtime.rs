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

//! # Container Runtime Abstraction
//!
//! This module provides a trait-based abstraction for container runtimes,
//! allowing DevCon to work with different container CLIs (Apple's container,
//! Docker, Podman, etc.).

use std::path::Path;

pub mod apple;
pub mod docker;

/// Trait for container runtime implementations.
///
/// This trait defines the interface for interacting with container runtimes,
/// allowing DevCon to work with different container CLIs transparently.
pub trait ContainerRuntime {
    /// Builds a container image from a Dockerfile.
    ///
    /// # Arguments
    ///
    /// * `dockerfile_path` - Path to the Dockerfile
    /// * `context_path` - Build context directory path
    /// * `image_tag` - Tag to apply to the built image
    ///
    /// # Errors
    ///
    /// Returns an error if the build command fails.
    fn build(
        &self,
        dockerfile_path: &Path,
        context_path: &Path,
        image_tag: &str,
    ) -> anyhow::Result<()>;

    /// Starts a container instance.
    ///
    /// # Arguments
    ///
    /// * `image_tag` - Image to run
    /// * `volume_mount` - Volume mount in format "host_path:container_path"
    /// * `label` - Label in format "key=value"
    /// * `env_vars` - Environment variables to set
    ///
    /// # Errors
    ///
    /// Returns an error if the run command fails.
    fn run(
        &self,
        image_tag: &str,
        volume_mount: &str,
        label: &str,
        env_vars: &[String],
    ) -> anyhow::Result<()>;

    /// Executes a command in a running container.
    ///
    /// # Arguments
    ///
    /// * `container_id` - ID of the container
    /// * `command` - Command to execute (e.g., shell path)
    /// * `env_vars` - Environment variables to set
    ///
    /// # Errors
    ///
    /// Returns an error if the exec command fails.
    fn exec(&self, container_id: &str, command: &str, env_vars: &[String]) -> anyhow::Result<()>;

    /// Lists running containers.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing (container_name, container_id) pairs.
    /// The container_name is extracted from the "devcon" label.
    ///
    /// # Errors
    ///
    /// Returns an error if the list command fails or output cannot be parsed.
    fn list(&self) -> anyhow::Result<Vec<(String, String)>>;
}
