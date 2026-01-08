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

//! # Container Driver
//!
//! This module provides the `ContainerDriver` for building and managing
//! development container lifecycles.
//!
//! ## Overview
//!
//! The `ContainerDriver` handles:
//! - Building container images from devcontainer configurations
//! - Generating Dockerfiles with feature installations
//! - Starting containers with appropriate volume mounts
//!
//! ## Usage
//!
//! ```no_run
//! use devcon::devcontainer::Devcontainer;
//! use devcon::driver::container::ContainerDriver;
//! use std::path::PathBuf;
//!
//! # fn example() -> anyhow::Result<()> {
//! let config = Devcontainer::try_from(PathBuf::from("/path/to/project"))?;
//! let driver = ContainerDriver::new(&config);
//!
//! // Build the container image
//! driver.build()?;
//!
//! // Start the container
//! driver.start(PathBuf::from("/path/to/project"))?;
//! # Ok(())
//! # }
//! ```

use std::{
    fs::{self, File},
    path::PathBuf,
    process::Command,
};

use anyhow::bail;
use tempfile::TempDir;

use crate::{devcontainer::Devcontainer, driver::process_features};

/// Driver for managing container build and runtime operations.
///
/// This struct encapsulates the logic for building container images
/// and starting container instances based on devcontainer configurations.
///
/// # Lifetime
///
/// The driver holds a reference to a `Devcontainer` configuration,
/// so it must not outlive the configuration.
pub struct ContainerDriver<'a> {
    devcontainer: &'a Devcontainer,
}

impl<'a> ContainerDriver<'a> {
    /// Creates a new `ContainerDriver` instance.
    ///
    /// # Arguments
    ///
    /// * `devcontainer` - Reference to the devcontainer configuration
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::devcontainer::Devcontainer;
    /// # use devcon::driver::container::ContainerDriver;
    /// # use std::path::PathBuf;
    /// # fn example() -> anyhow::Result<()> {
    /// let config = Devcontainer::try_from(PathBuf::from("/project"))?;
    /// let driver = ContainerDriver::new(&config);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(devcontainer: &'a Devcontainer) -> Self {
        Self { devcontainer }
    }

    /// Builds a container image from the devcontainer configuration.
    ///
    /// This method:
    /// 1. Creates a temporary directory for the build context
    /// 2. Downloads and extracts all configured features
    /// 3. Generates a Dockerfile with feature installations and dotfiles setup
    /// 4. Builds the container image using the `container` CLI tool
    ///
    /// The resulting image is tagged as `devcon-{container_name}`.
    ///
    /// # Arguments
    ///
    /// * `dotfiles_repo` - Optional URL to a dotfiles repository to clone
    /// * `env_variables` - Environment variables to set in the container
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Feature processing fails
    /// - Dockerfile generation fails
    /// - Container build command fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::devcontainer::Devcontainer;
    /// # use devcon::driver::container::ContainerDriver;
    /// # use std::path::PathBuf;
    /// # fn example() -> anyhow::Result<()> {
    /// let config = Devcontainer::try_from(PathBuf::from("/project"))?;
    /// let driver = ContainerDriver::new(&config);
    /// driver.build(Some("https://github.com/user/dotfiles"), &["EDITOR=vim".to_string()])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(
        &self,
        path: PathBuf,
        dotfiles_repo: Option<&str>,
        env_variables: &[String],
    ) -> anyhow::Result<()> {
        let directory = TempDir::new()?;

        let processed_features = process_features(&self.devcontainer.features, &directory)?;
        let mut feature_install = String::new();

        for (feature, feature_path) in processed_features {
            let feature_name = match &feature.source {
                crate::devcontainer::FeatureSource::Registry {
                    registry_type: _,
                    registry,
                } => &registry.name,
                crate::devcontainer::FeatureSource::Local { path } => {
                    &path.to_string_lossy().to_string()
                }
            };
            feature_install.push_str(&format!(
                "COPY {}/* /tmp/features/{}/ \n",
                feature_path, feature_name
            ));
            feature_install.push_str(&format!(
                "RUN chmod +x /tmp/features/{}/install.sh && /tmp/features/{}/install.sh \n",
                feature_name, feature_name
            ));
        }

        // Add environment variables
        let mut env_setup = String::new();
        for env_var in env_variables {
            env_setup.push_str(&format!("ENV {}\n", env_var));
        }

        // Determine remote user
        let remote_user = self.devcontainer.remote_user.as_deref().unwrap_or("root");

        // Add dotfiles setup if repository is provided
        let dotfiles_setup = if let Some(repo) = dotfiles_repo {
            format!(
                "RUN cd && git clone {} .dotfiles && cd .dotfiles && ./setup.sh || true\n",
                repo
            )
        } else {
            String::new()
        };

        let dockerfile = directory.path().join("Dockerfile");
        File::create(&dockerfile)?;

        let contents = format!(
            r#"
FROM {}
RUN mkdir /tmp/features
{}{}
USER {}
{}

WORKDIR /workspaces/{}
CMD ["sleep", "infinity"]
    "#,
            self.devcontainer.image,
            feature_install,
            env_setup,
            remote_user,
            dotfiles_setup,
            path.file_name().unwrap().to_string_lossy()
        );

        fs::write(&dockerfile, contents)?;

        let result = Command::new("container")
            .arg("build")
            .arg("-f")
            .arg(&dockerfile)
            .arg("-t")
            .arg(self.get_image_tag())
            .arg(directory.path())
            .status();

        if result.is_err() {
            bail!("Failed to build container image")
        }

        if result.unwrap().code() != Some(0) {
            bail!("Container build command failed")
        }

        directory.close()?;
        Ok(())
    }

    /// Starts a container instance with the project directory mounted.
    ///
    /// This method starts a container in detached mode with:
    /// - The project directory mounted at `/workspaces/{project_name}`
    /// - Environment variables from the config
    /// - Automatic removal on exit (`--rm` flag)
    /// - Detached mode (`-d` flag)
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the project directory to mount
    /// * `env_variables` - Environment variables to pass to the container
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The container image doesn't exist (must run `build()` first)
    /// - The container CLI command fails
    /// - The path is invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::devcontainer::Devcontainer;
    /// # use devcon::driver::container::ContainerDriver;
    /// # use std::path::PathBuf;
    /// # fn example() -> anyhow::Result<()> {
    /// let config = Devcontainer::try_from(PathBuf::from("/project"))?;
    /// let driver = ContainerDriver::new(&config);
    /// driver.build(None, &[])?;
    /// driver.start(PathBuf::from("/project"), &["EDITOR=vim".to_string()])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn start(&self, path: PathBuf, env_variables: &[String]) -> anyhow::Result<()> {
        let mut cmd = Command::new("container");
        cmd.arg("run")
            .arg("--rm")
            .arg("-d")
            .arg("-v")
            .arg(format!(
                "{}:/workspaces/{}",
                path.to_string_lossy(),
                path.file_name().unwrap().to_string_lossy()
            ))
            .arg("-l")
            .arg(format!(
                "devcon={}",
                path.file_name().unwrap().to_string_lossy()
            ));

        // Add environment variables
        for env_var in env_variables {
            if env_var.contains("=") {
                cmd.arg("-e").arg(env_var);
            } else {
                // Read host env variable
                let host_value = std::env::var(env_var).unwrap_or_default();
                cmd.arg("-e").arg(format!("{}={}", env_var, host_value));
            }
        }

        cmd.arg(self.get_image_tag());

        let result = cmd.status();

        if result.is_err() {
            bail!("Failed to start container")
        }

        if result.unwrap().code() != Some(0) {
            bail!("Container start command failed")
        }

        Ok(())
    }

    /// Shells into a started container.
    ///
    /// This method executes a shell within the container. The env variables
    /// from the config will be passed as shell envs.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the project directory to mount
    /// * `env_variables` - Environment variables to pass to the container
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The container image doesn't exist (must run `build()` first)
    /// - The container CLI command fails
    /// - The path is invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use devcon::devcontainer::Devcontainer;
    /// # use devcon::driver::container::ContainerDriver;
    /// # use std::path::PathBuf;
    /// # fn example() -> anyhow::Result<()> {
    /// let config = Devcontainer::try_from(PathBuf::from("/project"))?;
    /// let driver = ContainerDriver::new(&config);
    /// driver.build(None, &[])?;
    /// driver.shell(PathBuf::from("/project"), &["EDITOR=vim".to_string()])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn shell(
        &self,
        path: PathBuf,
        env_variables: &[String],
        default_shell: Option<String>,
    ) -> anyhow::Result<()> {
        let name = path.file_name().unwrap().to_string_lossy();
        let containers = ContainerDriver::list()?;

        let container_id = containers
            .iter()
            .find(|(container_name, _)| container_name == &name)
            .map(|(_, id)| id.clone());

        println!("Connecting to container for project: {}", name);
        println!("ContainerID: {:?}", container_id);
        if container_id.is_none() {
            bail!("No running container found for project {}", name);
        }

        let mut cmd = Command::new("container");
        cmd.arg("exec").arg("-it");

        for env_var in env_variables {
            if env_var.contains("=") {
                cmd.arg("-e").arg(env_var);
            } else {
                // Read host env variable
                let host_value = std::env::var(env_var).unwrap_or_default();
                cmd.arg("-e").arg(format!("{}={}", env_var, host_value));
            }
        }

        let result = cmd
            .arg(container_id.unwrap())
            .arg(default_shell.unwrap_or("zsh".to_string()))
            .status()?;

        if result.code() != Some(0) {
            bail!("Container exec command failed")
        }
        Ok(())
    }

    fn list() -> anyhow::Result<Vec<(String, String)>> {
        let output = Command::new("container")
            .arg("list")
            .arg("--format")
            .arg("json")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let containers: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;

        let result: Vec<(String, String)> = containers
            .iter()
            .map(|container| {
                let name = container["configuration"]["labels"]["devcon"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let id = container["configuration"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                (name, id)
            })
            .collect();

        Ok(result)
    }

    /// Returns the Docker image tag for this container.
    ///
    /// The tag is formatted as `devcon-{container_name}` where the container
    /// name is either the configured name or "default".
    ///
    /// # Returns
    ///
    /// A string containing the full image tag.
    fn get_image_tag(&self) -> String {
        format!("devcon-{}", self.devcontainer.get_computed_name())
    }
}
