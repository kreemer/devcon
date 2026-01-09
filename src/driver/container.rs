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

use std::fs::{self, File};

use anyhow::bail;
use minijinja::Environment;
use tempfile::TempDir;

use crate::{
    devcontainer::DevcontainerWorkspace,
    devcontainer::LifecycleCommand,
    driver::{process_features, runtime::ContainerRuntime},
};

/// Driver for managing container build and runtime operations.
///
/// This struct encapsulates the logic for building container images
/// and starting container instances based on devcontainer configurations.
pub struct ContainerDriver {
    runtime: Box<dyn ContainerRuntime>,
}

impl ContainerDriver {
    /// Creates a new ContainerDriver with the specified runtime.
    pub fn new(runtime: Box<dyn ContainerRuntime>) -> Self {
        Self { runtime }
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
        devcontainer_workspace: DevcontainerWorkspace,
        env_variables: &[String],
    ) -> anyhow::Result<()> {
        let directory = TempDir::new()?;
        println!(
            "Building container in temporary directory: {}",
            directory.path().to_string_lossy()
        );
        let processed_features =
            process_features(&devcontainer_workspace.devcontainer.features, &directory)?;
        let mut feature_install = String::new();

        let mut i = 0;
        for feature_result in processed_features {
            let feature_name = match &feature_result.feature.source {
                crate::devcontainer::FeatureSource::Registry {
                    registry_type: _,
                    registry,
                } => &registry.name,
                crate::devcontainer::FeatureSource::Local { path } => {
                    &path.to_string_lossy().to_string()
                }
            };
            if i == 0 {
                feature_install.push_str(&format!("FROM {} AS feature_0 \n", "base"));
            } else {
                feature_install.push_str(&format!("FROM feature_{} AS feature_{} \n", i - 1, i));
            }
            feature_install.push_str(&format!(
                "COPY {}/* /tmp/features/{}/ \n",
                feature_result.relative_path, feature_name
            ));

            feature_install.push_str(&format!(
                "RUN chmod +x /tmp/features/{}/install.sh && . /tmp/features/{}/devcontainer-features.env && /tmp/features/{}/install.sh \n",
                feature_name, feature_name, feature_name
            ));

            i += 1;
        }

        feature_install.push_str(&format!("FROM feature_{} AS feature_last \n", i - 1));

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
ENV _REMOTE_USER={{ remote_user }}
ENV _CONTAINER_USER={{ container_user }}
ENV _REMOTE_USER_HOME={{ remote_user_home }}
ENV _CONTAINER_USER_HOME={{ container_user_home }}

RUN mkdir /tmp/features
{{ feature_install }}{{ env_setup }}

FROM feature_last AS dotfiles_setup
{{ dotfiles_setup }}

FROM dotfiles_setup
USER {{ remote_user }}
WORKDIR /workspaces/{{ workspace_name }}
CMD ["sleep", "infinity"]
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
        devcontainer_workspace: DevcontainerWorkspace,
        dotfiles_repo: Option<&str>,
        dotfiles_install_command: Option<&str>,
        env_variables: &[String],
    ) -> anyhow::Result<()> {
        let volume_mount = format!(
            "{}:/workspaces/{}",
            devcontainer_workspace.path.to_string_lossy(),
            devcontainer_workspace
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
        );

        let label = format!(
            "devcon={}",
            devcontainer_workspace
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
        );

        // Process environment variables
        let mut processed_env_vars = Vec::new();
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
        )?;

        match &devcontainer_workspace.devcontainer.on_create_command {
            Some(LifecycleCommand::String(cmd)) => {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &c.to_string()], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                self.runtime.exec(
                    handle.as_ref(),
                    vec!["/bin/sh", "-c", &cmd.to_string()],
                    &[],
                )
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        // Add dotfiles setup if repository is provided
        if let Some(repo) = dotfiles_repo {
            self.runtime.exec(
                handle.as_ref(),
                vec![
                    "/bin/sh",
                    "-c",
                    &format!(
                        "/dotfiles_helper.sh {} {}",
                        repo,
                        dotfiles_install_command.unwrap_or("")
                    ),
                ],
                &[],
            )?;
        };

        match &devcontainer_workspace.devcontainer.post_create_command {
            Some(LifecycleCommand::String(cmd)) => {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &c.to_string()], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                self.runtime.exec(
                    handle.as_ref(),
                    vec!["/bin/sh", "-c", &cmd.to_string()],
                    &[],
                )
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        match &devcontainer_workspace.devcontainer.post_start_command {
            Some(LifecycleCommand::String(cmd)) => {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &cmd], &[])?
            }
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                self.runtime
                    .exec(handle.as_ref(), vec!["/bin/sh", "-c", &c.to_string()], &[])
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                self.runtime.exec(
                    handle.as_ref(),
                    vec!["/bin/sh", "-c", &cmd.to_string()],
                    &[],
                )
            })?,
            None => { /* No onCreateCommand specified */ }
        };

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
        devcontainer_workspace: DevcontainerWorkspace,
        env_variables: &[String],
        default_shell: Option<String>,
    ) -> anyhow::Result<()> {
        let name = devcontainer_workspace
            .path
            .file_name()
            .unwrap()
            .to_string_lossy();
        let containers = self.runtime.list()?;

        let handle = containers
            .iter()
            .find(|(container_name, _)| container_name == &name)
            .map(|(_, id)| id);

        if handle.is_none() {
            bail!("No running container found for project {}", name);
        }

        // Process environment variables
        let mut processed_env_vars = Vec::new();
        for env_var in env_variables {
            if env_var.contains("=") {
                processed_env_vars.push(env_var.clone());
            } else {
                // Read host env variable
                let host_value = std::env::var(env_var).unwrap_or_default();
                processed_env_vars.push(format!("{}={}", env_var, host_value));
            }
        }

        match &devcontainer_workspace.devcontainer.post_attach_command {
            Some(LifecycleCommand::String(cmd)) => self.runtime.exec(
                handle.as_ref().unwrap().as_ref(),
                vec!["/bin/sh", "-c", &cmd],
                &[],
            )?,
            Some(LifecycleCommand::Array(cmds)) => cmds.iter().try_for_each(|c| {
                self.runtime.exec(
                    handle.as_ref().unwrap().as_ref(),
                    vec!["/bin/sh", "-c", &c.to_string()],
                    &[],
                )
            })?,
            Some(LifecycleCommand::Object(map)) => map.values().try_for_each(|cmd| {
                self.runtime.exec(
                    handle.as_ref().unwrap().as_ref(),
                    vec!["/bin/sh", "-c", &cmd.to_string()],
                    &[],
                )
            })?,
            None => { /* No onCreateCommand specified */ }
        };

        self.runtime.exec(
            handle.as_ref().unwrap().as_ref(),
            vec![&default_shell.unwrap_or("zsh".to_string())],
            &processed_env_vars,
        )?;

        Ok(())
    }

    /// Returns the Docker image tag for this container.
    ///
    /// The tag is formatted as `devcon-{container_name}` where the container
    /// name is either the configured name or "default".
    ///
    /// # Returns
    ///
    /// A string containing the full image tag.
    fn get_image_tag(&self, devcontainer_workspace: &DevcontainerWorkspace) -> String {
        format!("devcon-{}", devcontainer_workspace.get_sanitized_name())
    }
}
