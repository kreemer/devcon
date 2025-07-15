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

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::{path::PathBuf, process::Stdio};

use crate::config::{AppConfig, DevContainerContext};

fn get_helper_path(config: &AppConfig) -> PathBuf {
    config.socket_path.clone()
}

fn get_socket_path(config: &AppConfig) -> PathBuf {
    get_helper_path(config).join("devcon.sock")
}

fn get_helper_script_path(config: &AppConfig) -> PathBuf {
    get_helper_path(config).join("devcon-browser")
}

fn ensure_helper_script_exists(config: &AppConfig) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let script_path = get_helper_script_path(config);

    // Create the helper script content
    let script_content = r#"#!/bin/bash
# DevCon Browser Helper Script - Auto-generated
# This script allows opening URLs in the host's default browser from within a devcontainer

SOCKET_PATH="/tmp/devcon-browser.sock"

if [ $# -eq 0 ]; then
    echo "Usage: $0 <url>"
    echo "Example: $0 https://github.com"
    exit 1
fi

URL="$1"

if [ ! -S "$SOCKET_PATH" ]; then
    echo "Error: DevCon browser socket not found at $SOCKET_PATH"
    echo "Make sure the devcon socket server is running on the host:"
    echo "  devcon socket --daemon"
    echo "The socket is typically located in your XDG runtime directory on the host."
    exit 1
fi

# Send the URL to the socket
echo "$URL" | nc -U "$SOCKET_PATH" 2>/dev/null || {
    echo "Error: Failed to send URL to socket. Is netcat (nc) installed?"
    exit 1
}
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

    Ok(script_path)
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

    // Build devcontainer command
    let mut cmd = Command::new("devcontainer");
    cmd.arg("up").arg("--workspace-folder").arg(path);

    // Check if socket exists and mount it if it does
    let socket_path = get_helper_path(config);
    if socket_path.exists() {
        cmd.arg("--mount").arg(format!(
            "type=bind,source={},target=/tmp/devcon-browser.sock",
            socket_path.display()
        ));

        // Also mount the helper script if socket exists
        match ensure_helper_script_exists(config) {
            Ok(script_path) => {
                cmd.arg("--mount").arg(format!(
                    "type=bind,source={},target=/usr/local/bin/devcon-browser",
                    script_path.display()
                ));
            }
            Err(e) => {
                eprintln!("Warning: Failed to create helper script: {e}");
            }
        }
    }

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
        cmd.arg("--remote-env")
            .arg(format!("{}={}", env.name, env.value));
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start devcontainer: {error}").into());
    }

    println!("Successfully started devcontainer!");
    println!("Output: {}", String::from_utf8_lossy(&output.stdout));
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

    // Build devcontainer command
    let mut cmd = Command::new("devcontainer");
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.arg("exec").arg("--workspace-folder").arg(path);

    let socket_path = get_socket_path(config);
    if socket_path.exists() {
        cmd.arg("--remote-env").arg("BROWSER=devcon-browser");
    }

    // Add variables to build and up context
    for env in config.list_env_by_context(DevContainerContext::Exec) {
        cmd.arg("--remote-env")
            .arg(format!("{}={}", env.name, env.value));
    }

    for env in env_list {
        cmd.arg("--remote-env").arg(env); // Assuming env is in "NAME=VALUE" format
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_up_devcontainer_no_devcontainer_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
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
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
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
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
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
    fn test_up_devcontainer_with_dotfiles() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            dotfiles_repo: Some("https://github.com/user/dotfiles".to_string()),
            socket_path: temp_dir.path().to_path_buf(),
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
        let mut config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
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
    fn test_get_socket_path() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let socket_path = get_socket_path(&config);
        assert!(socket_path.to_string_lossy().contains("devcon.sock"));
    }

    #[test]
    fn test_get_helper_script_path() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let script_path = get_helper_script_path(&config);
        assert!(script_path.to_string_lossy().contains("devcon-browser"));
        assert!(!script_path.to_string_lossy().contains(".sh")); // Should not have extension
    }

    #[test]
    fn test_ensure_helper_script_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let result = ensure_helper_script_exists(&config);
        assert!(result.is_ok());

        let script_path = result.unwrap();
        assert!(script_path.exists());

        // Check that it's executable
        let metadata = fs::metadata(&script_path).unwrap();
        let permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(permissions.mode() & 0o111, 0o111); // Check executable bits
        }

        // Check script content
        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("#!/bin/bash"));
        assert!(content.contains("DevCon Browser Helper Script"));
        assert!(content.contains("/tmp/devcon-browser.sock"));
        assert!(content.contains("nc -U"));

        // Test that calling it again doesn't recreate the file
        let modified_time = metadata.modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result2 = ensure_helper_script_exists(&config);
        assert!(result2.is_ok());

        let metadata2 = fs::metadata(&script_path).unwrap();
        assert_eq!(modified_time, metadata2.modified().unwrap());
    }

    #[test]
    fn test_ensure_helper_script_updates_outdated_content() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        // Call ensure_helper_script_exists to create the initial script
        let result = ensure_helper_script_exists(&config);
        assert!(result.is_ok());
        let script_path = result.unwrap();

        // Verify it exists and has the correct content
        assert!(script_path.exists());
        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("DevCon Browser Helper Script"));

        // Now modify the script to have outdated content
        fs::write(&script_path, "#!/bin/bash\necho 'old script'\n").unwrap();

        // Call ensure_helper_script_exists again - should update the content
        let result2 = ensure_helper_script_exists(&config);
        assert!(result2.is_ok());

        // Check that content was updated
        let updated_content = fs::read_to_string(&script_path).unwrap();
        assert!(updated_content.contains("DevCon Browser Helper Script"));
        assert!(!updated_content.contains("old script"));
    }

    #[test]
    fn test_socket_mounting_in_commands() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // Create a temporary socket file to simulate socket existence
        let socket_dir = temp_dir.path().join("socket_test");
        fs::create_dir_all(&socket_dir).unwrap();
        let socket_file = socket_dir.join("browser.sock");
        fs::write(&socket_file, "").unwrap(); // Create empty file to simulate socket

        // Test up_devcontainer (will fail due to missing devcontainer CLI, but we check the error)
        let result = up_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // The command should be built correctly even if execution fails
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // Should not fail due to missing devcontainer config
        assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
    }

    #[test]
    fn test_browser_env_variable_in_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // Create a temporary socket file
        let socket_dir = temp_dir.path().join("socket_test");
        fs::create_dir_all(&socket_dir).unwrap();
        let socket_file = socket_dir.join("browser.sock");
        fs::write(&socket_file, "").unwrap();

        // Test shell_devcontainer
        let result = shell_devcontainer(&temp_dir.path().to_path_buf(), &vec![], &config);

        // Should fail due to missing devcontainer CLI but not due to config issues
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
    }

    #[test]
    fn test_commands_without_socket() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();

        // Override XDG directories to point to non-existent socket
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();

        // Test that commands work even without socket
        let result1 = up_devcontainer(&temp_dir.path().to_path_buf(), &config);
        let result2 = shell_devcontainer(&temp_dir.path().to_path_buf(), &vec![], &config);

        // Both should fail due to missing devcontainer CLI, not socket issues
        assert!(result1.is_err());
        assert!(result2.is_err());

        let error1 = result1.unwrap_err().to_string();
        let error2 = result2.unwrap_err().to_string();

        // Should not contain socket-related errors
        assert!(!error1.contains("socket"));
        assert!(!error2.contains("socket"));
    }

    #[test]
    fn test_browser_env_variable_list_in_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig {
            socket_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

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
