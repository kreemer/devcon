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

use minijinja::{Environment, Error, render};
use pidfile::PidFile;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::{path::PathBuf, process::Stdio};

use crate::config::{AppConfig, DevContainerContext, RuntimeConfig};

fn ensure_helper_script_exists() -> Result<(), Box<dyn std::error::Error>> {
    let xdg_dirs = xdg::BaseDirectories::with_prefix("devcon");
    let socket_directory = xdg_dirs
        .get_data_home()
        .expect("Failed to create XDG base directories");

    let script_path = socket_directory.join("devcon-browser.sh");

    // Create the helper script content
    let script_content = r#"#!/bin/bash
# DevCon Browser Helper Script - Auto-generated
# This script allows opening URLs in the host's default browser from within a devcontainer

if [ $# -eq 0 ]; then
    echo "Usage: $0 <url>"
    echo "Example: $0 https://github.com"
    exit 1
fi

URL="$1"

# Send the URL to the socket
devcon-client $DEVCON_SERVER browser --url "$URL"
"#;

    // Write the script if it doesn't exist or is outdated
    if !script_path.exists()
        || fs::read_to_string(&script_path).map_or(true, |content| content != script_content)
    {
        // Ensure parent directory exists
        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&script_path, script_content)?;

        // Make it executable
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }

    Ok(())
}

fn exec_minijinja(command: String, arguments: Vec<String>) -> Result<String, Error> {
    let mut cmd = Command::new(command);
    for argument in arguments {
        cmd.arg(argument.clone());
    }

    let output = cmd.output().map_err(|e| {
        Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "Could not execute command",
        )
        .with_source(e)
    })?;

    if !output.clone().status.success() {
        return Err(Error::new(minijinja::ErrorKind::InvalidOperation, "Error"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout.to_string().trim().to_string())
}

pub fn up_devcontainer(
    path: &PathBuf,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the path has a .devcontainer directory or devcontainer.json
    let devcontainer_dir = path.join(".devcontainer");
    let devcontainer_file = path.join("devcontainer.json");

    if !devcontainer_dir.exists() && !devcontainer_file.exists() {
        return Err(format!(
            "No .devcontainer directory or devcontainer.json found in {}",
            path.display()
        )
        .into());
    }

    let mut environment = Environment::new();
    environment.add_function("exec", exec_minijinja);

    // Build devcontainer command
    let mut cmd = Command::new("devcontainer");
    cmd.arg("up").arg("--workspace-folder").arg(path);

    // Add dotfiles repository if configured
    if let Some(ref dotfiles_repo) = config.dotfiles_repo {
        cmd.arg("--dotfiles-repository").arg(dotfiles_repo);
    }

    // Add additional features if configured
    if !config.additional_features.is_empty() {
        let additional_features_string: &String = &config
            .additional_features
            .iter()
            .map(|(f, v)| format!("\"{f}\": {v}"))
            .collect::<Vec<String>>()
            .join(", ");
        cmd.arg("--additional-features")
            .arg(format!("{{ {additional_features_string} }}"));
    }

    // Add variables to build and up context
    for env in config.list_env_by_context(DevContainerContext::Up) {
        cmd.arg("--remote-env").arg(format!(
            "{}={}",
            env.name,
            render!(in environment, &env.value)
        ));
    }

    let xdg_dirs = xdg::BaseDirectories::with_prefix("devcon");
    let socket_directory = xdg_dirs
        .get_data_home()
        .expect("Failed to create XDG base directories");

    if ensure_helper_script_exists().is_ok() {
        let socket_script = socket_directory.join("devcon-browser.sh");
        cmd.arg("--mount").arg(format!(
            "type=bind,source={},target=/opt/devcon-browser.sh",
            socket_script.display()
        ));
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start devcontainer: {error}").into());
    }

    let json: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
        .expect("JSON was not well-formatted");

    if let Some(container_id) = json.get("containerId") {
        println!("Container with id {} started", container_id);
    }

    Ok(())
}

pub fn shell_devcontainer(
    path: &PathBuf,
    env_list: &Vec<String>,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the path has a .devcontainer directory or devcontainer.json
    let devcontainer_dir = path.join(".devcontainer");
    let devcontainer_file = path.join("devcontainer.json");

    if !devcontainer_dir.exists() && !devcontainer_file.exists() {
        return Err(format!(
            "No .devcontainer directory or devcontainer.json found in {}",
            path.display()
        )
        .into());
    }

    let mut environment = Environment::new();
    environment.add_function("exec", exec_minijinja);

    // Build devcontainer command
    let mut cmd = Command::new("devcontainer");
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.arg("exec").arg("--workspace-folder").arg(path);

    // Add variables to build and up context
    for env in config.list_env_by_context(DevContainerContext::Exec) {
        cmd.arg("--remote-env").arg(format!(
            "{}={}",
            env.name,
            render!(in environment, &env.value)
        ));
    }

    for env in env_list {
        cmd.arg("--remote-env").arg(env); // Assuming env is in "NAME=VALUE" format
    }

    let pidfile_path = xdg::BaseDirectories::with_prefix("devcon")
        .get_state_home()
        .expect("Failed to create XDG base directories");

    let runtime_config = read_runtime_config();
    if PidFile::is_locked(&pidfile_path.join("devcon.pid")).unwrap_or(false)
        && runtime_config.is_ok()
    {
        cmd.arg("--remote-env")
            .arg("BROWSER=/opt/devcon-browser.sh");

        cmd.arg("--remote-env").arg(format!(
            "DEVCON_SERVER=host.docker.internal:{}",
            runtime_config?
                .socket_address
                .map(|f| f.to_string())
                .unwrap_or("".to_string())
        ));
    }

    cmd.arg("zsh");

    let output = cmd.output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to up devcontainer: {error}").into());
    }

    Ok(())
}

pub fn check_devcontainer_cli() -> Result<(), Box<dyn std::error::Error>> {
    // Check if devcontainer CLI is available
    let output = Command::new("devcontainer").arg("--version").output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("✅ DevContainer CLI version: {}", version.trim());
            Ok(())
        },
        Ok(_) => Err("DevContainer CLI is installed but not working properly".into()),
        Err(_) => Err("DevContainer CLI is not installed or not in PATH. Please install it with: npm install -g @devcontainers/cli".into()),
    }
}

fn read_runtime_config() -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
    let runtime_config_path = xdg::BaseDirectories::with_prefix("devcon")
        .get_config_file("runtime.yaml")
        .ok_or("No runtime config")?;

    let reader = std::fs::File::open(runtime_config_path)?;
    let runtime_config: RuntimeConfig = serde_yaml::from_reader(reader)?;

    Ok(runtime_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_up_devcontainer_no_devcontainer_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig::default();
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No .devcontainer directory or devcontainer.json found")
        );
    }

    #[test]
    fn test_up_devcontainer_with_devcontainer_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig::default();
        let devcontainer_path = temp_dir.path().join(".devcontainer");
        fs::create_dir(&devcontainer_path).unwrap();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can't easily test the actual command upution without devcontainer CLI installed
        // but we can test that it doesn't fail due to missing .devcontainer directory
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_up_devcontainer_with_devcontainer_json() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig::default();
        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_up_devcontainer_with_dotfiles() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            dotfiles_repo: Some("https://github.com/user/dotfiles".to_string()),
            ..Default::default()
        };
        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_up_devcontainer_with_additional_features() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = AppConfig::default();
        config.additional_features.insert(
            "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            "latest".to_string(),
        );
        config.additional_features.insert(
            "ghcr.io/devcontainers/features/docker-in-docker:2".to_string(),
            "20.10".to_string(),
        );

        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_browser_env_variable_list_in_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig::default();

        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // Create a temporary socket file
        let socket_dir = temp_dir.path().join("socket_test");
        fs::create_dir_all(&socket_dir).unwrap();
        let socket_file = socket_dir.join("browser.sock");
        fs::write(&socket_file, "").unwrap();

        // Test shell_devcontainer
        let result = shell_devcontainer(
            &temp_dir.path().to_path_buf(),
            &vec!["A=B".to_string(), "C=D".to_string()],
            &config,
        );

        // Should fail due to missing devcontainer CLI but not due to config issues
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
    }
}
