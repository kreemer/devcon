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

//! # Command Handlers
//!
//! This module contains command handler functions that process user commands
//! and orchestrate the execution of various DevCon operations.
//!
//! Each handler function corresponds to a CLI subcommand and is responsible for:
//! - Loading and parsing devcontainer configuration
//! - Loading user configuration from XDG directories
//! - Merging configuration settings
//! - Creating necessary driver instances
//! - Executing the requested operation
//! - Handling errors and returning results

use std::io::Write;
use std::{
    fs::{self, File},
    path::PathBuf,
};

use crate::{config::Config, devcontainer::Devcontainer, driver::container::ContainerDriver};

/// Handles the config command to display the config file path.
///
/// This function prints the path to the DevCon configuration file,
/// which is typically located at `~/.config/devcon/config.yaml`.
///
/// # Errors
///
/// Returns an error if the config directory cannot be determined.
///
/// # Examples
///
/// ```no_run
/// # use devcon::command::handle_config_command;
/// handle_config_command()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn handle_config_command(create_if_missing: bool) -> anyhow::Result<()> {
    let config_path = Config::get_config_path()?;

    if !config_path.exists() && create_if_missing {
        let config_dir = config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
        fs::create_dir_all(config_dir)?;
        let mut file = File::create(&config_path)?;

        let documentation = r#"# DevCon Configuration File
# This file contains user-specific settings for DevCon.
# Modify the values below to customize your DevCon experience.
#
# dotfiles_repository: https://github.com/user/dotfiles.git
# additional_features:
#   ghcr.io/someowner/somerepo/somefeature:latest:
#     option1: value1
#     option2: value2
# env_variables:
#   - VAR1=value1
#   - LOCALENV
"#;

        file.write_all(documentation.as_bytes())?;

        println!("Config file created at {}", config_path.display());
        return Ok(());
    }

    println!("{}", config_path.display());
    Ok(())
}

/// Handles the build command for creating a development container.
///
/// This function:
/// 1. Loads the user configuration from XDG directories
/// 2. Loads the devcontainer configuration from the specified path
/// 3. Merges additional features from user config
/// 4. Creates a `ContainerDriver` instance
/// 5. Builds the container image with all configured features
///
/// # Arguments
///
/// * `path` - The path to the project directory containing `.devcontainer/devcontainer.json`
///
/// # Errors
///
/// Returns an error if:
/// - The devcontainer configuration cannot be found or parsed
/// - Additional features cannot be merged
/// - The container build process fails
/// - Required dependencies are missing
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// # use devcon::command::handle_build_command;
///
/// let project_path = PathBuf::from("/path/to/project");
/// handle_build_command(project_path)?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn handle_build_command(path: PathBuf) -> anyhow::Result<()> {
    let canonical_path = std::fs::canonicalize(&path)?;
    let config = Config::load()?;
    let mut devcontainer = Devcontainer::try_from(path)?;

    // Merge additional features from config
    devcontainer.merge_additional_features(&config.additional_features)?;

    let driver = ContainerDriver::new(&devcontainer);
    let result = driver.build(canonical_path, config.dotfiles_repository.as_deref(), &[]);

    if result.is_err() {
        println!("Error: {:?}", result.err());
        anyhow::bail!("Failed to build the development container.");
    }

    Ok(())
}

/// Handles the start command for launching a development container.
///
/// This function:
/// 1. Loads the user configuration from XDG directories
/// 2. Loads the devcontainer configuration from the specified path
/// 3. Resolves the canonical path to the project directory
/// 4. Creates a `ContainerDriver` instance
/// 5. Starts the container with the project mounted as a volume and env variables
///
/// # Arguments
///
/// * `path` - The path to the project directory containing `.devcontainer/devcontainer.json`
///
/// # Errors
///
/// Returns an error if:
/// - The devcontainer configuration cannot be found or parsed
/// - The path cannot be canonicalized
/// - The container image doesn't exist (must be built first)
/// - The container fails to start
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// # use devcon::command::handle_start_command;
///
/// let project_path = PathBuf::from("/path/to/project");
/// handle_start_command(project_path)?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn handle_start_command(path: PathBuf) -> anyhow::Result<()> {
    let devcontainer = Devcontainer::try_from(path.clone())?;
    let canonical_path = std::fs::canonicalize(&path)?;
    let driver = ContainerDriver::new(&devcontainer);
    driver.start(canonical_path, &[])?;

    Ok(())
}

/// Handles the shell command for opening a shell in a running container.
///
/// # Arguments
///
/// * `path` - Path to the project directory
/// * `_env` - Environment variables to pass to the shell (currently unused)
///
/// # Errors
///
/// Currently always returns `Ok(())` as it's not implemented.
pub fn handle_shell_command(path: PathBuf, _env: &[String]) -> anyhow::Result<()> {
    let config = Config::load()?;
    let devcontainer = Devcontainer::try_from(path.clone())?;
    let canonical_path = std::fs::canonicalize(&path)?;
    let driver = ContainerDriver::new(&devcontainer);
    driver.shell(canonical_path, &config.env_variables, config.default_shell)?;
    Ok(())
}
