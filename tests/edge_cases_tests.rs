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

use devcon::{AppConfig, ConfigManager};
use std::env;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_command_error_handling() {
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

    // Test opening non-existent directory
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "open",
            "/non/existent/path",
        ])
        .output()
        .expect("Failed to execute devcon open");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("does not exist"));

    // Test shell command with non-existent directory
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "shell",
            "/non/existent/path",
        ])
        .output()
        .expect("Failed to execute devcon shell");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("does not exist"));

    // Test config commands with invalid input
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "remove",
            "999",
        ])
        .output()
        .expect("Failed to execute devcon config envs remove");

    assert!(!output.status.success());

    // Test config envs add with invalid context
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "TEST",
            "value",
            "invalid_context",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");

    // Should succeed but use default context (the parsing error is handled internally)
    assert!(output.status.success());
}

#[test]
fn test_comprehensive_config_workflow() {
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

    // Test complete workflow: dotfiles -> features -> envs

    // 1. Set dotfiles
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "dotfiles",
            "set",
            "https://github.com/user/dotfiles",
        ])
        .output()
        .expect("Failed to execute devcon config dotfiles set");
    assert!(output.status.success());

    // 2. Add features
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "add",
            "ghcr.io/devcontainers/features/github-cli:1",
            "latest",
        ])
        .output()
        .expect("Failed to execute devcon config features add");
    assert!(output.status.success());

    // 3. Add environment variables
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "EDITOR",
            "vim",
            "all",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");
    assert!(output.status.success());

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "DEBUG",
            "true",
            "exec",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");
    assert!(output.status.success());

    // 4. Verify configuration
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "dotfiles",
            "show",
        ])
        .output()
        .expect("Failed to execute devcon config dotfiles show");
    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(output_text.contains("https://github.com/user/dotfiles"));

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "list",
        ])
        .output()
        .expect("Failed to execute devcon config features list");
    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(output_text.contains("github-cli"));

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
    assert!(output_text.contains("EDITOR"));
    assert!(output_text.contains("DEBUG"));

    // 5. Clean up
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "dotfiles",
            "clear",
        ])
        .output()
        .expect("Failed to execute devcon config dotfiles clear");
    assert!(output.status.success());

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "clear",
        ])
        .output()
        .expect("Failed to execute devcon config features clear");
    assert!(output.status.success());

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "clear",
        ])
        .output()
        .expect("Failed to execute devcon config envs clear");
    assert!(output.status.success());

    // 6. Verify cleanup
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "dotfiles",
            "show",
        ])
        .output()
        .expect("Failed to execute devcon config dotfiles show");
    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(output_text.contains("No dotfiles repository configured"));

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "list",
        ])
        .output()
        .expect("Failed to execute devcon config features list");
    assert!(output.status.success());
    let output_text = String::from_utf8(output.stdout).unwrap();
    assert!(output_text.contains("No additional features configured"));

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
    assert!(output_text.contains("No additional env vars found"));
}

#[test]
fn test_help_and_version_commands() {
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

    // Test main help
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "--help",
        ])
        .output()
        .expect("Failed to execute devcon --help");
    assert!(output.status.success());
    let help_text = String::from_utf8(output.stdout).unwrap();
    assert!(help_text.contains("DevCon helps you manage"));

    // Test version
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "--version",
        ])
        .output()
        .expect("Failed to execute devcon --version");
    assert!(output.status.success());

    // Test subcommand help
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "--help",
        ])
        .output()
        .expect("Failed to execute devcon config --help");
    assert!(output.status.success());
    let help_text = String::from_utf8(output.stdout).unwrap();
    assert!(help_text.contains("dotfiles"));
    assert!(help_text.contains("features"));
    assert!(help_text.contains("envs"));
}

#[test]
fn test_check_command() {
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

    // Test check command (will likely fail due to missing devcontainer CLI)
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "check",
        ])
        .output()
        .expect("Failed to execute devcon check");

    // The command should run and provide feedback
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.contains("Checking system requirements") || stderr.contains("DevContainer CLI"));
}
