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

use devcon::AppConfig;
use devcon::ConfigManager;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_browser_integration_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
    let config = AppConfig {
        socket_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    config_manager.save_config(&config).unwrap();

    // Create devcon binary path
    let devcon_binary = env::current_dir()
        .unwrap()
        .join("target")
        .join("release")
        .join("devcon");

    if !devcon_binary.exists() {
        println!("Skipping integration test - devcon binary not found at {devcon_binary:?}",);
        return;
    }

    // Test socket --show-path
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
        ])
        .args(["socket", "--show-path"])
        .output()
        .expect("Failed to execute devcon socket --show-path");

    assert!(output.status.success());
    let socket_path = String::from_utf8(output.stdout).unwrap().trim().to_string();

    assert!(socket_path.contains("devcon.sock"));

    // Verify that the socket path is a valid path
    let socket_path_buf = PathBuf::from(&socket_path);
    assert!(socket_path_buf.parent().is_some());
}

#[test]
fn test_helper_script_creation() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
    let config = AppConfig {
        socket_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    config_manager.save_config(&config).unwrap();

    // Create a mock devcontainer project
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir_all(&project_dir).unwrap();

    let devcontainer_dir = project_dir.join(".devcontainer");
    fs::create_dir_all(&devcontainer_dir).unwrap();

    let devcontainer_json = devcontainer_dir.join("devcontainer.json");
    fs::write(
        &devcontainer_json,
        r#"{"name": "Test", "image": "ubuntu:20.04"}"#,
    )
    .unwrap();

    // Create a socket file to simulate running socket server
    let socket_path = temp_dir.path().join("devcon.sock");
    fs::write(&socket_path, "").unwrap(); // Empty file to simulate socket

    let devcon_binary = env::current_dir()
        .unwrap()
        .join("target")
        .join("release")
        .join("devcon");

    if !devcon_binary.exists() {
        println!("Skipping integration test - devcon binary not found");
        return;
    }

    // Test devcon open (will fail due to missing devcontainer CLI, but helper script should be created)
    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "open",
            project_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute devcon open");

    // Check if helper script was created
    let helper_script_path = temp_dir.path().join("devcon-browser");
    assert!(
        helper_script_path.exists(),
        "Helper script should be created at {helper_script_path:?}",
    );

    // Check script content
    let script_content = fs::read_to_string(&helper_script_path).unwrap();
    assert!(script_content.contains("#!/bin/bash"));
    assert!(script_content.contains("DevCon Browser Helper Script"));
    assert!(script_content.contains("/tmp/devcon-browser.sock"));

    // Check that script is executable
    let metadata = fs::metadata(&helper_script_path).unwrap();
    let permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            permissions.mode() & 0o111,
            0o111,
            "Script should be executable"
        );
    }
}

#[test]
fn test_socket_command_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
    let config = AppConfig {
        socket_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    config_manager.save_config(&config).unwrap();

    let devcon_binary = env::current_dir()
        .unwrap()
        .join("target")
        .join("release")
        .join("devcon");

    if !devcon_binary.exists() {
        println!("Skipping integration test - devcon binary not found");
        return;
    }

    // Test invalid socket command
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "socket",
            "--invalid-flag",
        ])
        .output()
        .expect("Failed to execute devcon socket with invalid flag");

    assert!(!output.status.success());

    // Test help output
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "socket",
            "--help",
        ])
        .output()
        .expect("Failed to execute devcon socket --help");

    assert!(output.status.success());
    let help_text = String::from_utf8(output.stdout).unwrap();
    assert!(help_text.contains("socket server"));
    assert!(help_text.contains("--daemon"));
    assert!(help_text.contains("--show-path"));
}

#[test]
fn test_environment_variable_context_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path().join("config.yaml")).unwrap();
    let config = AppConfig {
        socket_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };
    config_manager.save_config(&config).unwrap();

    let devcon_binary = env::current_dir()
        .unwrap()
        .join("target")
        .join("release")
        .join("devcon");

    if !devcon_binary.exists() {
        println!("Skipping integration test - devcon binary not found");
        return;
    }

    // Test adding environment variables with different contexts
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "TEST_VAR",
            "test_value",
            "exec",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8(output.stderr).unwrap()
    );

    // Test listing environment variables
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "list",
        ])
        .output()
        .expect("Failed to execute devcon config envs list");

    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(output_text.contains("TEST_VAR"), "{output_text}");
    assert!(output_text.contains("test_value"), "{output_text}");
    assert!(output_text.contains("Exec"), "{output_text}");

    // Test removing environment variable
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "remove",
            "0",
        ])
        .output()
        .expect("Failed to execute devcon config envs remove");

    assert!(output.status.success());

    // Verify it was removed
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "list",
        ])
        .output()
        .expect("Failed to execute devcon config envs list");

    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(
        output_text.contains("No additional env vars found"),
        "{output_text}"
    );
}
