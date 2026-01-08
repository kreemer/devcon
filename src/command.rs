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
//! - Creating necessary driver instances
//! - Executing the requested operation
//! - Handling errors and returning results

use std::path::PathBuf;

use crate::{devcontainer::Devcontainer, driver::container::ContainerDriver};

/// Handles the build command for creating a development container.
///
/// This function:
/// 1. Loads the devcontainer configuration from the specified path
/// 2. Creates a `ContainerDriver` instance
/// 3. Builds the container image with all configured features
///
/// # Arguments
///
/// * `path` - The path to the project directory containing `.devcontainer/devcontainer.json`
///
/// # Errors
///
/// Returns an error if:
/// - The devcontainer configuration cannot be found or parsed
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
    let devcontainer = Devcontainer::try_from(path)?;
    let driver = ContainerDriver::new(&devcontainer);
    driver.build()?;

    Ok(())
}

/// Handles the start command for launching a development container.
///
/// This function:
/// 1. Loads the devcontainer configuration from the specified path
/// 2. Resolves the canonical path to the project directory
/// 3. Creates a `ContainerDriver` instance
/// 4. Starts the container with the project mounted as a volume
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
    driver.start(canonical_path)?;

    Ok(())
}

/// Handles the shell command for opening a shell in a running container.
///
/// **Note:** This function is currently a placeholder and not yet implemented.
///
/// # Arguments
///
/// * `_path` - Optional path to the project directory (currently unused)
/// * `_env` - Environment variables to pass to the shell (currently unused)
///
/// # Errors
///
/// Currently always returns `Ok(())` as it's not implemented.
pub fn handle_shell_command(_path: Option<&PathBuf>, _env: &[String]) -> anyhow::Result<()> {
    Ok(())
}
