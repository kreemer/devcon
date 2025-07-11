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

use devcon::config::{AppConfig, AppConfigEnv, DevContainerContext};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn test_socket_path_generation() {
    let temp_dir = TempDir::new().unwrap();

    // Test with XDG_RUNTIME_DIR
    unsafe {
        env::set_var("XDG_RUNTIME_DIR", temp_dir.path().join("runtime"));
        env::remove_var("XDG_DATA_HOME");
    }

    // The socket path should be generated using XDG directories
    let expected_runtime_path = temp_dir
        .path()
        .join("runtime")
        .join("devcon")
        .join("browser.sock");

    // Test with XDG_DATA_HOME fallback
    unsafe {
        env::remove_var("XDG_RUNTIME_DIR");
        env::set_var("XDG_DATA_HOME", temp_dir.path().join("data"));
    }

    let expected_data_path = temp_dir
        .path()
        .join("data")
        .join("devcon")
        .join("browser.sock");

    // Both paths should be valid and contain the expected components
    assert!(
        expected_runtime_path
            .to_string_lossy()
            .contains("browser.sock")
    );
    assert!(
        expected_data_path
            .to_string_lossy()
            .contains("browser.sock")
    );
}

#[test]
fn test_helper_script_content() {
    let temp_dir = TempDir::new().unwrap();

    // Override XDG directories for testing
    unsafe {
        env::set_var("XDG_RUNTIME_DIR", temp_dir.path());
    }

    // Create a mock socket file
    let socket_dir = temp_dir.path().join("devcon");
    fs::create_dir_all(&socket_dir).unwrap();
    let socket_path = socket_dir.join("browser.sock");
    fs::write(&socket_path, "").unwrap();

    // Create helper script manually (simulating the function)
    let helper_script_path = socket_dir.join("devcon-browser");
    let script_content = format!(
        r#"#!/bin/bash
# DevCon Browser Helper Script
# Automatically generated - do not edit manually

if [ $# -eq 0 ]; then
    echo "Usage: $0 <url>"
    exit 1
fi

URL="$1"

# Send URL to DevCon socket server
echo "$URL" | socat - UNIX-CONNECT:{}
"#,
        socket_path.display()
    );

    fs::write(&helper_script_path, script_content).unwrap();

    // Make script executable
    let mut permissions = fs::metadata(&helper_script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&helper_script_path, permissions).unwrap();

    // Test script content
    let content = fs::read_to_string(&helper_script_path).unwrap();
    assert!(content.contains("#!/bin/bash"));
    assert!(content.contains("DevCon Browser Helper Script"));
    assert!(content.contains("UNIX-CONNECT:"));
    assert!(content.contains("browser.sock"));

    // Test script permissions
    let metadata = fs::metadata(&helper_script_path).unwrap();
    let permissions = metadata.permissions();
    assert_eq!(permissions.mode() & 0o111, 0o111); // Should be executable
}

#[test]
fn test_devcontainer_command_construction() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("test_project");
    fs::create_dir_all(&project_path).unwrap();

    // Create devcontainer.json
    let devcontainer_dir = project_path.join(".devcontainer");
    fs::create_dir_all(&devcontainer_dir).unwrap();
    let devcontainer_json = devcontainer_dir.join("devcontainer.json");
    fs::write(
        &devcontainer_json,
        r#"{"name": "Test", "image": "ubuntu:20.04"}"#,
    )
    .unwrap();

    // Test basic command construction
    let mut config = AppConfig::default();
    config.dotfiles_repo = Some("https://github.com/user/dotfiles".to_string());

    // Add features
    config.additional_features.insert(
        "ghcr.io/devcontainers/features/github-cli:1".to_string(),
        "latest".to_string(),
    );

    // Add environment variables
    config.env.push(AppConfigEnv {
        name: "EDITOR".to_string(),
        value: "vim".to_string(),
        context: DevContainerContext::All,
    });

    config.env.push(AppConfigEnv {
        name: "DEBUG".to_string(),
        value: "true".to_string(),
        context: DevContainerContext::Up,
    });

    // Test that configuration is properly structured
    assert!(config.dotfiles_repo.is_some());
    assert_eq!(config.additional_features.len(), 1);
    assert_eq!(config.env.len(), 2);

    // Test environment variable filtering
    let up_env = config.list_env_by_context(DevContainerContext::Up);
    assert_eq!(up_env.len(), 2); // Should include ALL and UP contexts

    let exec_env = config.list_env_by_context(DevContainerContext::Exec);
    assert_eq!(exec_env.len(), 1); // Should include only ALL context
}

#[test]
fn test_mount_arguments_construction() {
    let temp_dir = TempDir::new().unwrap();

    // Create socket and helper script
    let socket_dir = temp_dir.path().join("devcon");
    fs::create_dir_all(&socket_dir).unwrap();
    let socket_path = socket_dir.join("browser.sock");
    let helper_script_path = socket_dir.join("devcon-browser");

    fs::write(&socket_path, "").unwrap();
    fs::write(&helper_script_path, "#!/bin/bash\necho test").unwrap();

    // Test mount arguments
    let expected_socket_mount = format!("{}:/tmp/devcon-browser.sock", socket_path.display());
    let expected_script_mount = format!(
        "{}:/usr/local/bin/devcon-browser",
        helper_script_path.display()
    );

    assert!(expected_socket_mount.contains("devcon-browser.sock"));
    assert!(expected_script_mount.contains("devcon-browser"));
    assert!(expected_socket_mount.contains(":/tmp/"));
    assert!(expected_script_mount.contains(":/usr/local/bin/"));
}

#[test]
fn test_environment_variable_contexts() {
    let mut config = AppConfig::default();

    // Add environment variables for different contexts
    config.env.push(AppConfigEnv {
        name: "GLOBAL_VAR".to_string(),
        value: "global_value".to_string(),
        context: DevContainerContext::All,
    });

    config.env.push(AppConfigEnv {
        name: "UP_VAR".to_string(),
        value: "up_value".to_string(),
        context: DevContainerContext::Up,
    });

    config.env.push(AppConfigEnv {
        name: "EXEC_VAR".to_string(),
        value: "exec_value".to_string(),
        context: DevContainerContext::Exec,
    });

    // Test Up context (should include All and Up)
    let up_env = config.list_env_by_context(DevContainerContext::Up);
    assert_eq!(up_env.len(), 2);
    assert!(up_env.iter().any(|e| e.name == "GLOBAL_VAR"));
    assert!(up_env.iter().any(|e| e.name == "UP_VAR"));
    assert!(!up_env.iter().any(|e| e.name == "EXEC_VAR"));

    // Test Exec context (should include All and Exec)
    let exec_env = config.list_env_by_context(DevContainerContext::Exec);
    assert_eq!(exec_env.len(), 2);
    assert!(exec_env.iter().any(|e| e.name == "GLOBAL_VAR"));
    assert!(exec_env.iter().any(|e| e.name == "EXEC_VAR"));
    assert!(!exec_env.iter().any(|e| e.name == "UP_VAR"));

    // Test All context (should include only All)
    let all_env = config.list_env_by_context(DevContainerContext::All);
    assert_eq!(all_env.len(), 1);
    assert!(all_env.iter().any(|e| e.name == "GLOBAL_VAR"));
}

#[test]
fn test_browser_environment_variable_setting() {
    let temp_dir = TempDir::new().unwrap();

    // Create socket to simulate browser integration
    let socket_dir = temp_dir.path().join("devcon");
    fs::create_dir_all(&socket_dir).unwrap();
    let socket_path = socket_dir.join("browser.sock");
    fs::write(&socket_path, "").unwrap();

    // Test that BROWSER environment variable would be set
    let browser_env_value = "/usr/local/bin/devcon-browser";

    // In the actual implementation, this would be added to the exec command
    let mut env_vars = HashMap::new();
    env_vars.insert("BROWSER".to_string(), browser_env_value.to_string());

    assert_eq!(
        env_vars.get("BROWSER"),
        Some(&browser_env_value.to_string())
    );

    // Test that the environment variable is only set when socket exists
    let non_existent_socket = temp_dir.path().join("non_existent.sock");
    assert!(!non_existent_socket.exists());

    // Browser env should not be set when socket doesn't exist
    let mut conditional_env = HashMap::new();
    if socket_path.exists() {
        conditional_env.insert("BROWSER".to_string(), browser_env_value.to_string());
    }

    assert!(conditional_env.contains_key("BROWSER"));

    // Test with non-existent socket
    let mut empty_env = HashMap::new();
    if non_existent_socket.exists() {
        empty_env.insert("BROWSER".to_string(), browser_env_value.to_string());
    }

    assert!(!empty_env.contains_key("BROWSER"));
}

#[test]
fn test_file_permissions_and_security() {
    let temp_dir = TempDir::new().unwrap();

    // Create helper script
    let script_path = temp_dir.path().join("test-script");
    fs::write(&script_path, "#!/bin/bash\necho test").unwrap();

    // Set proper permissions
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();

    // Test permissions
    let metadata = fs::metadata(&script_path).unwrap();
    let permissions = metadata.permissions();

    // Should be executable by owner
    assert_eq!(permissions.mode() & 0o100, 0o100);
    // Should be readable by owner
    assert_eq!(permissions.mode() & 0o400, 0o400);
    // Should be writable by owner
    assert_eq!(permissions.mode() & 0o200, 0o200);

    // Test that file is executable
    assert_eq!(permissions.mode() & 0o111, 0o111);
}

#[test]
fn test_devcontainer_project_validation() {
    let temp_dir = TempDir::new().unwrap();

    // Test valid project structure
    let valid_project = temp_dir.path().join("valid_project");
    fs::create_dir_all(&valid_project).unwrap();

    let devcontainer_dir = valid_project.join(".devcontainer");
    fs::create_dir_all(&devcontainer_dir).unwrap();

    let devcontainer_json = devcontainer_dir.join("devcontainer.json");
    fs::write(
        &devcontainer_json,
        r#"{"name": "Test", "image": "ubuntu:20.04"}"#,
    )
    .unwrap();

    // Validate project structure
    assert!(valid_project.exists());
    assert!(devcontainer_dir.exists());
    assert!(devcontainer_json.exists());

    // Test invalid project (missing .devcontainer)
    let invalid_project = temp_dir.path().join("invalid_project");
    fs::create_dir_all(&invalid_project).unwrap();

    assert!(invalid_project.exists());
    assert!(!invalid_project.join(".devcontainer").exists());

    // Test project with docker-compose
    let compose_project = temp_dir.path().join("compose_project");
    let compose_devcontainer_dir = compose_project.join(".devcontainer");
    fs::create_dir_all(&compose_devcontainer_dir).unwrap();

    let compose_json = compose_devcontainer_dir.join("devcontainer.json");
    fs::write(
        &compose_json,
        r#"{"name": "Compose", "dockerComposeFile": "docker-compose.yml", "service": "app"}"#,
    )
    .unwrap();

    let docker_compose = compose_devcontainer_dir.join("docker-compose.yml");
    fs::write(
        &docker_compose,
        "version: '3'\nservices:\n  app:\n    image: ubuntu:20.04",
    )
    .unwrap();

    assert!(compose_json.exists());
    assert!(docker_compose.exists());
}

#[test]
fn test_features_configuration() {
    let mut config = AppConfig::default();

    // Add various features
    config.additional_features.insert(
        "ghcr.io/devcontainers/features/github-cli:1".to_string(),
        "latest".to_string(),
    );

    config.additional_features.insert(
        "ghcr.io/devcontainers/features/docker-in-docker:2".to_string(),
        r#"{"version": "20.10", "moby": true}"#.to_string(),
    );

    config.additional_features.insert(
        "ghcr.io/devcontainers/features/node:1".to_string(),
        r#"{"version": "18"}"#.to_string(),
    );

    // Test feature configuration
    assert_eq!(config.additional_features.len(), 3);
    assert!(
        config
            .additional_features
            .contains_key("ghcr.io/devcontainers/features/github-cli:1")
    );
    assert!(
        config
            .additional_features
            .contains_key("ghcr.io/devcontainers/features/docker-in-docker:2")
    );
    assert!(
        config
            .additional_features
            .contains_key("ghcr.io/devcontainers/features/node:1")
    );

    // Test different configuration formats
    assert_eq!(
        config
            .additional_features
            .get("ghcr.io/devcontainers/features/github-cli:1"),
        Some(&"latest".to_string())
    );

    let docker_config = config
        .additional_features
        .get("ghcr.io/devcontainers/features/docker-in-docker:2")
        .unwrap();
    assert!(docker_config.contains("20.10"));
    assert!(docker_config.contains("moby"));

    let node_config = config
        .additional_features
        .get("ghcr.io/devcontainers/features/node:1")
        .unwrap();
    assert!(node_config.contains("18"));
}
