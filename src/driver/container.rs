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

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, bail};
use devcon_proto::{AgentMessage, agent_message};
use dircpy::copy_dir;
use minijinja::Environment;
use prost::Message as _;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tracing::{debug, info, trace, warn};

use crate::devcontainer::{FeatureRef, FeatureSource};
use crate::driver::agent::{self, AgentConfig};
use crate::driver::feature_process::FeatureProcessResult;
use crate::{
    config::Config,
    devcontainer::{LifecycleCommand, OnAutoForward, PortAttributes},
    driver::feature_process::process_features,
    driver::runtime::ContainerRuntime,
    workspace::Workspace,
};

/// Driver for managing container build and runtime operations.
///
/// This struct encapsulates the logic for building container images
/// and starting container instances based on devcontainer configurations.
pub struct ContainerDriver {
    config: Config,
    runtime: Box<dyn ContainerRuntime>,
}

impl ContainerDriver {
    /// Creates a new ContainerDriver with the specified runtime.
    pub fn new(config: Config, runtime: Box<dyn ContainerRuntime>) -> Self {
        Self { config, runtime }
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
        devcontainer_workspace: Workspace,
        env_variables: &[String],
    ) -> anyhow::Result<()> {
        let directory = TempDir::new()?;
        info!(
            "Building container in temporary directory: {}",
            directory.path().to_string_lossy()
        );

        trace!(
            "Processing features for devcontainer at {:?}",
            devcontainer_workspace.path
        );

        trace!(
            "Using features of devcontainer: {:?}",
            devcontainer_workspace.devcontainer.features
        );
        trace!(
            "Adding additional features from config: {:?}",
            self.config.additional_features
        );

        // Merge additional features from config
        let mut features = devcontainer_workspace
            .devcontainer
            .merge_additional_features(&self.config.additional_features)?;

        // Add agent installation feature
        let agent_path = agent::Agent::new(AgentConfig::default()).generate()?;
        features.push(FeatureRef::new(FeatureSource::Local { path: agent_path }));

        debug!("Final feature list: {:?}", features);
        let processed_features = process_features(&features)?;
        let mut feature_install = String::new();

        // Collect mounts from all features
        let mut feature_mounts = Vec::new();
        for feature_result in &processed_features {
            if let Some(ref mounts) = feature_result.feature.mounts {
                // Convert feature::FeatureMount to devcontainer::Mount
                for mount in mounts {
                    match mount {
                        crate::feature::FeatureMount::String(s) => {
                            feature_mounts.push(crate::devcontainer::Mount::String(s.clone()));
                        }
                        crate::feature::FeatureMount::Structured(sm) => {
                            let mount_type = match sm.mount_type {
                                crate::feature::MountType::Bind => {
                                    crate::devcontainer::MountType::Bind
                                }
                                crate::feature::MountType::Volume => {
                                    crate::devcontainer::MountType::Volume
                                }
                            };
                            feature_mounts.push(crate::devcontainer::Mount::Structured(
                                crate::devcontainer::StructuredMount {
                                    mount_type,
                                    source: sm.source.clone(),
                                    target: sm.target.clone(),
                                },
                            ));
                        }
                    }
                }
            }
        }

        let mut i = 0;
        for feature_result in processed_features {
            let feature_path_name = self.copy_feature_to_build(&feature_result, &directory)?;
            let feature_name = match &feature_result.feature_ref.source {
                FeatureSource::Registry { registry } => &registry.name,
                FeatureSource::Local { path } => &path
                    .canonicalize()?
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            };
            if i == 0 {
                feature_install.push_str(&format!("FROM {} AS feature_0 \n", "base"));
            } else {
                feature_install.push_str(&format!("FROM feature_{} AS feature_{} \n", i - 1, i));
            }
            feature_install.push_str(&format!(
                "COPY {}/* /tmp/features/{}/ \n",
                feature_path_name, feature_name
            ));

            feature_install.push_str(&format!(
                "RUN chmod +x /tmp/features/{}/install.sh && . /tmp/features/{}/devcontainer-features.env && /tmp/features/{}/install.sh \n",
                feature_name, feature_name, feature_name
            ));

            i += 1;
        }
        if i > 0 {
            feature_install.push_str(&format!("FROM feature_{} AS feature_last \n", i - 1));
        } else {
            feature_install.push_str("FROM base AS feature_last \n");
        }

        // Add environment variables
        let mut env_setup = String::new();
        for env_var in env_variables {
            env_setup.push_str(&format!("ENV {}\n", env_var));
        }

        // Add dotfiles setup if repository is provided
        let dotfiles_setup = {
            let dotfiles_helper_path = directory.path().join("dotfiles_helper.sh");
            let dotfiles_helper_content = r#"
#!/bin/sh
set -e
cd && git clone $1 .dotfiles && cd .dotfiles
if [ -n "$2" ]; then
    chmod +x $2
    ./$2 || true
else
    for f in install.sh setup.sh bootstrap.sh script/install.sh script/setup.sh script/bootstrap.sh
    do
        if [ -e $f ]
        then
            installCommand=$f
            break
        fi
    done

    if [ -n "$installCommand" ]; then
        chmod +x $installCommand
        ./$installCommand || true
    fi
fi
"#;

            fs::write(&dotfiles_helper_path, dotfiles_helper_content)?;
            "COPY dotfiles_helper.sh /dotfiles_helper.sh \nRUN chmod +x /dotfiles_helper.sh"
                .to_string()
        };

        let dockerfile = directory.path().join("Dockerfile");
        File::create(&dockerfile)?;

        let env = Environment::new();
        let template = env.template_from_str(
            r#"
FROM {{ image }} AS base
ENV DEVCON=true
ENV DEVCON_WORKSPACE_NAME={{ workspace_name }}
ENV _REMOTE_USER={{ remote_user }}
ENV _CONTAINER_USER={{ container_user }}
ENV _REMOTE_USER_HOME={{ remote_user_home }}
ENV _CONTAINER_USER_HOME={{ container_user_home }}

USER root
RUN mkdir /tmp/features
{{ feature_install }}
{{ env_setup }}

FROM feature_last AS dotfiles_setup
{{ dotfiles_setup }}

FROM dotfiles_setup
USER {{ remote_user }}
WORKDIR /workspaces/{{ workspace_name }}
ENTRYPOINT [ "/bin/sh" ]
CMD ["-c", "echo Container started\ntrap \"exit 0\" 15\n\nexec \"$@\"\nwhile sleep 1 \u0026 wait $!; do :; done", "-"]
"#,
        )?;

        let remote_user_val = devcontainer_workspace
            .devcontainer
            .remote_user
            .as_deref()
            .unwrap_or("vscode");
        let container_user_val = devcontainer_workspace
            .devcontainer
            .container_user
            .as_deref()
            .unwrap_or("vscode");
        let container_user_home = if container_user_val == "root" {
            "/root".to_string()
        } else {
            format!("/home/{}", container_user_val)
        };
        let remote_user_home = if remote_user_val == "root" {
            "/root".to_string()
        } else {
            format!("/home/{}", remote_user_val)
        };

        let contents = template.render(minijinja::context! {
            image => &devcontainer_workspace.devcontainer.image,
            remote_user => remote_user_val,
            container_user => container_user_val,
            remote_user_home => remote_user_home,
            container_user_home => container_user_home,
            feature_install => &feature_install,
            dotfiles_setup => &dotfiles_setup,
            env_setup => &env_setup,
            workspace_name => devcontainer_workspace.path.file_name().unwrap().to_string_lossy(),
        })?;

        fs::write(&dockerfile, contents)?;

        self.runtime.build(
            &dockerfile,
            directory.path(),
            &self.get_image_tag(&devcontainer_workspace),
        )?;

        directory.close()?;
        Ok(())
    }

    fn copy_feature_to_build(
        &self,
        process: &FeatureProcessResult,
        build_directory: &TempDir,
    ) -> anyhow::Result<String> {
        let directory_name = format!(
            "{}-{}",
            process.path.file_name().unwrap().to_string_lossy(),
            process.feature.version
        );
        let feature_dest = build_directory.path().join(&directory_name);
        copy_dir(&process.path, &feature_dest)?;

        Ok(feature_dest
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string())
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
    pub fn start(
        &self,
        devcontainer_workspace: Workspace,
        env_variables: &[String],
    ) -> anyhow::Result<()> {
        let handles = self.runtime.list()?;
        let handle = handles
            .iter()
            .find(|(name, _)| name == &self.get_container_name(&devcontainer_workspace));

        if handle.is_some() {
            return Ok(());
        }

        let images = self.runtime.images()?;
        let already_built = images.iter().any(|image| {
            image == &format!("{}:latest", self.get_image_tag(&devcontainer_workspace))
        });

        if !already_built {
            self.build(devcontainer_workspace.clone(), &[])?;
        }

        let volume_mount = format!(
            "{}:/workspaces/{}",
            devcontainer_workspace.path.to_string_lossy(),
            devcontainer_workspace
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
        );

        let label = self.get_container_label(&devcontainer_workspace);

        // Collect all mounts: from devcontainer config and features
        let mut all_mounts = Vec::new();

        // Add mounts from devcontainer configuration
        if let Some(ref mounts) = devcontainer_workspace.devcontainer.mounts {
            all_mounts.extend(mounts.clone());
        }

        // Process features to get their mounts
        let mut features = devcontainer_workspace
            .devcontainer
            .merge_additional_features(&self.config.additional_features)?;

        // Add agent installation feature
        let agent_path = agent::Agent::new(AgentConfig::default()).generate()?;
        features.push(FeatureRef::new(FeatureSource::Local { path: agent_path }));

        // Extract mounts from features (we need to process them but won't use the full results here)
        let processed_features = process_features(&features)?;
        for feature_result in &processed_features {
            if let Some(ref mounts) = feature_result.feature.mounts {
                // Convert feature::FeatureMount to devcontainer::Mount
                for mount in mounts {
                    match mount {
                        crate::feature::FeatureMount::String(s) => {
                            all_mounts.push(crate::devcontainer::Mount::String(s.clone()));
                        }
                        crate::feature::FeatureMount::Structured(sm) => {
                            let mount_type = match sm.mount_type {
                                crate::feature::MountType::Bind => {
                                    crate::devcontainer::MountType::Bind
                                }
                                crate::feature::MountType::Volume => {
                                    crate::devcontainer::MountType::Volume
                                }
                            };
                            all_mounts.push(crate::devcontainer::Mount::Structured(
                                crate::devcontainer::StructuredMount {
                                    mount_type,
                                    source: sm.source.clone(),
                                    target: sm.target.clone(),
                                },
                            ));
                        }
                    }
                }
            }
        }

        // Process environment variables
        let mut processed_env_vars = Vec::new();

        // Add DEVCON_SOCKET environment variable
        let socket_path = format!(
            "/tmp/devcon-sockets/devcon-{}.sock",
            devcontainer_workspace.get_sanitized_name()
        );
        processed_env_vars.push(format!("DEVCON_SOCKET={}", socket_path));

        for env_var in env_variables {
            if env_var.contains("=") {
                processed_env_vars.push(env_var.clone());
            } else {
                // Read host env variable
                let host_value = std::env::var(env_var).unwrap_or_default();
                processed_env_vars.push(format!("{}={}", env_var, host_value));
            }
        }

        let handle = self.runtime.run(
            &self.get_image_tag(&devcontainer_workspace),
            &volume_mount,
            &label,
            &processed_env_vars,
            &all_mounts,
        )?;

        match &devcontainer_workspace.devcontainer.on_create_command {
            Some(LifecycleCommand::String(cmd)) => {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, cmd);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, c);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                let cmd_str = cmd.to_command_string();
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, &cmd_str);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        // Add dotfiles setup if repository is provided
        if let Some(repo) = self.config.dotfiles_repository.as_deref() {
            self.runtime.exec(
                handle.as_ref(),
                vec![
                    "/bin/sh",
                    "-c",
                    &format!(
                        "/dotfiles_helper.sh {} {}",
                        repo,
                        self.config
                            .dotfiles_install_command
                            .as_deref()
                            .unwrap_or("")
                    ),
                ],
                &[],
            )?;
        };

        match &devcontainer_workspace.devcontainer.post_create_command {
            Some(LifecycleCommand::String(cmd)) => {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, cmd);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, c);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                let cmd_str = cmd.to_command_string();
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, &cmd_str);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        match &devcontainer_workspace.devcontainer.post_start_command {
            Some(LifecycleCommand::String(cmd)) => {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, cmd);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, c);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                let cmd_str = cmd.to_command_string();
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, &cmd_str);
                self.runtime
                    .exec(handle.as_ref(), vec!["bash", "-c", "-i", &wrapped_cmd], &[])
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        // Start listener for agent messages
        self.start_agent_listener(handle)?;

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
    pub fn shell(&self, devcontainer_workspace: Workspace) -> anyhow::Result<()> {
        let handles = self.runtime.list()?;
        let handle = handles
            .iter()
            .find(|(name, _)| name == &self.get_container_name(&devcontainer_workspace));

        if handle.is_none() {
            println!(
                "No running container found for project {}, starting one...",
                devcontainer_workspace
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            );
            self.start(devcontainer_workspace.clone(), &[])?;
        }

        let name = devcontainer_workspace
            .path
            .file_name()
            .unwrap()
            .to_string_lossy();
        let containers = self.runtime.list()?;

        let handle = containers
            .iter()
            .find(|(container_name, _)| {
                container_name == &self.get_container_name(&devcontainer_workspace)
            })
            .map(|(_, id)| id);

        if handle.is_none() {
            bail!("No running container found for project {}", name);
        }

        // Process environment variables
        let mut processed_env_vars = Vec::new();
        for env_var in self.config.env_variables.iter() {
            if env_var.contains("=") {
                processed_env_vars.push(env_var.clone());
            } else {
                // Read host env variable
                let host_value = std::env::var(env_var).unwrap_or_default();
                processed_env_vars.push(format!("{}={}", env_var, host_value));
            }
        }

        match &devcontainer_workspace.devcontainer.post_attach_command {
            Some(LifecycleCommand::String(cmd)) => {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, cmd);
                self.runtime.exec(
                    handle.as_ref().unwrap().as_ref(),
                    vec!["bash", "-c", "-i", &wrapped_cmd],
                    &[],
                )?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, c);
                self.runtime.exec(
                    handle.as_ref().unwrap().as_ref(),
                    vec!["bash", "-c", "-i", &wrapped_cmd],
                    &[],
                )
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                let cmd_str = cmd.to_command_string();
                let wrapped_cmd = self.wrap_lifecycle_command(&devcontainer_workspace, &cmd_str);
                self.runtime.exec(
                    handle.as_ref().unwrap().as_ref(),
                    vec!["bash", "-c", "-i", &wrapped_cmd],
                    &[],
                )
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        self.runtime.exec(
            handle.as_ref().unwrap().as_ref(),
            vec![&self.config.default_shell.as_deref().unwrap_or("zsh")],
            &processed_env_vars,
        )?;

        Ok(())
    }

    /// Returns the Docker image tag for this container.
    ///
    /// The tag is formatted as `devcon-{sanitized_name}` where the sanitized
    /// name is the project directory name with special characters replaced.
    ///
    /// # Returns
    ///
    /// A string containing the full image tag.
    fn get_image_tag(&self, devcontainer_workspace: &Workspace) -> String {
        format!("devcon-{}", devcontainer_workspace.get_sanitized_name())
    }

    /// Returns the container name for this devcontainer.
    ///
    /// The name is formatted as `devcon.{sanitized_name}` where the sanitized
    /// name is the project directory name with special characters replaced.
    ///
    /// # Returns
    ///
    /// A string containing the container name.
    fn get_container_name(&self, devcontainer_workspace: &Workspace) -> String {
        format!("devcon.{}", devcontainer_workspace.get_sanitized_name())
    }

    /// Returns the container label for this devcontainer.
    ///
    /// The label is formatted as `devcon.project={sanitized_name}`.
    ///
    /// # Returns
    ///
    /// A string containing the label key-value pair.
    fn get_container_label(&self, devcontainer_workspace: &Workspace) -> String {
        format!(
            "devcon.project={}",
            devcontainer_workspace.get_sanitized_name()
        )
    }

    /// Wraps a lifecycle command with proper environment and working directory setup.
    ///
    /// This ensures the command runs with:
    /// - Proper shell environment loaded
    /// - Correct working directory
    /// - User's profile sourced
    ///
    /// # Arguments
    ///
    /// * `_devcontainer_workspace` - The devcontainer workspace
    /// * `cmd` - The command to wrap
    ///
    /// # Returns
    ///
    /// A wrapped command string ready for execution.
    fn wrap_lifecycle_command(&self, _devcontainer_workspace: &Workspace, cmd: &str) -> String {
        cmd.to_string()
    }

    /// Starts a background listener that reads and processes agent messages from Unix socket
    fn start_agent_listener(
        &self,
        container_handle: Box<dyn crate::driver::runtime::ContainerHandle>,
    ) -> anyhow::Result<()> {
        info!("Starting agent message listener on Unix socket...");

        use std::os::unix::net::UnixListener;

        // Create socket path in host's /tmp/devcon-sockets directory
        let socket_path = format!("/tmp/devcon-sockets/devcon-{}.sock", container_handle.id());

        // Create directory if it doesn't exist
        std::fs::create_dir_all("/tmp/devcon-sockets")?;

        // Remove existing socket if present
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)
            .context(format!("Failed to bind Unix socket at {}", socket_path))?;

        info!("Agent listener socket bound at {}", socket_path);

        let handle = thread::spawn(move || {
            debug!("Agent listener thread started, waiting for connections...");

            // Accept connections and process messages
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        debug!("Agent connected to socket");

                        // Read length-prefixed messages
                        let mut len_buf = [0u8; 4];
                        loop {
                            // Read message length
                            if let Err(e) = stream.read_exact(&mut len_buf) {
                                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                    debug!("Agent connection closed");
                                    break;
                                }
                                warn!("Error reading message length: {}", e);
                                break;
                            }

                            let len = u32::from_be_bytes(len_buf) as usize;
                            if len == 0 || len > 1024 * 1024 {
                                warn!("Invalid message length: {}", len);
                                break;
                            }

                            // Read message data
                            let mut msg_buf = vec![0u8; len];
                            if let Err(e) = stream.read_exact(&mut msg_buf) {
                                warn!("Error reading message data: {}", e);
                                break;
                            }

                            // Decode protobuf message
                            match AgentMessage::decode(&msg_buf[..]) {
                                Ok(msg) => {
                                    debug!("Received agent message: {:?}", msg);
                                    if let Some(message) = msg.message {
                                        match message {
                                            agent_message::Message::StartPortForward(req) => {
                                                info!(
                                                    "🔍 Agent discovered port {} listening in container",
                                                    req.port
                                                );
                                                // TODO: Implement actual port forwarding logic here
                                                info!(
                                                    "💡 Add port {} to 'forwardPorts' in devcontainer.json to auto-forward",
                                                    req.port
                                                );
                                            }
                                            agent_message::Message::StopPortForward(req) => {
                                                debug!("Port {} closed in container", req.port);
                                            }
                                            agent_message::Message::OpenUrl(req) => {
                                                debug!("Received OpenUrl request: {}", req.url);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to decode agent message: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to accept connection: {}", e);
                    }
                }
            }

            debug!("Agent listener thread exiting");
        });

        let result = handle.join();

        match result {
            Ok(_) => debug!("Agent listener thread joined successfully"),
            Err(e) => warn!("Agent listener thread panicked: {:?}", e),
        }

        Ok(())
    }
}
