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

use devcon::ConfigManager;
use devcon::config::{AppConfig, AppConfigEnv, DevContainerContext};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use tempfile::TempDir;

#[test]
fn test_dev_container_context_parsing() {
    // Test DevContainerContext FromStr implementation
    assert_eq!(
        DevContainerContext::from_str("all").unwrap(),
        DevContainerContext::All
    );
    assert_eq!(
        DevContainerContext::from_str("up").unwrap(),
        DevContainerContext::Up
    );
    assert_eq!(
        DevContainerContext::from_str("exec").unwrap(),
        DevContainerContext::Exec
    );

    // Test invalid context
    assert!(DevContainerContext::from_str("invalid").is_err());

    // Test case sensitivity
    assert_eq!(
        DevContainerContext::from_str("ALL").unwrap(),
        DevContainerContext::All
    );
    assert_eq!(
        DevContainerContext::from_str("UP").unwrap(),
        DevContainerContext::Up
    );
    assert_eq!(
        DevContainerContext::from_str("EXEC").unwrap(),
        DevContainerContext::Exec
    );
}

#[test]
fn test_app_config_env_filtering() {
    let mut config = AppConfig::default();

    // Add different environment variables with different contexts
    config.env.push(AppConfigEnv {
        name: "ALL_VAR".to_string(),
        value: "all_value".to_string(),
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

    // Test filtering by context
    let all_env = config.list_env_by_context(DevContainerContext::All);
    assert_eq!(all_env.len(), 1);
    assert_eq!(all_env[0].name, "ALL_VAR");

    let up_env = config.list_env_by_context(DevContainerContext::Up);
    assert_eq!(up_env.len(), 2); // Should include ALL_VAR and UP_VAR
    assert!(up_env.iter().any(|e| e.name == "ALL_VAR"));
    assert!(up_env.iter().any(|e| e.name == "UP_VAR"));

    let exec_env = config.list_env_by_context(DevContainerContext::Exec);
    assert_eq!(exec_env.len(), 2); // Should include ALL_VAR and EXEC_VAR
    assert!(exec_env.iter().any(|e| e.name == "ALL_VAR"));
    assert!(exec_env.iter().any(|e| e.name == "EXEC_VAR"));
}

#[test]
fn test_config_serialization() {
    let mut config = AppConfig::default();

    // Add sample data
    config.recent_paths.push(PathBuf::from("/path/to/project"));
    config.dotfiles_repo = Some("https://github.com/user/dotfiles".to_string());
    config
        .additional_features
        .insert("feature1".to_string(), "value1".to_string());
    config.env.push(AppConfigEnv {
        name: "TEST_VAR".to_string(),
        value: "test_value".to_string(),
        context: DevContainerContext::Exec,
    });

    // Test serialization
    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("recent_paths"));
    assert!(yaml.contains("/path/to/project"));
    assert!(yaml.contains("dotfiles_repo"));
    assert!(yaml.contains("additional_features"));
    assert!(yaml.contains("env"));
    assert!(yaml.contains("TEST_VAR"));

    // Test deserialization
    let deserialized: AppConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.recent_paths.len(), 1);
    assert_eq!(
        deserialized.recent_paths[0],
        PathBuf::from("/path/to/project")
    );
    assert_eq!(
        deserialized.dotfiles_repo,
        Some("https://github.com/user/dotfiles".to_string())
    );
    assert_eq!(deserialized.additional_features.len(), 1);
    assert_eq!(deserialized.env.len(), 1);
    assert_eq!(deserialized.env[0].name, "TEST_VAR");
}

#[test]
fn test_config_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");
    let config = AppConfig {
        socket_path: temp_dir.path().to_path_buf(),
        recent_paths: vec![PathBuf::from("/test/path")],
        dotfiles_repo: Some("https://github.com/test/dotfiles".to_string()),
        ..Default::default()
    };

    // Save config
    let yaml = serde_yaml::to_string(&config).unwrap();
    fs::write(&config_path, yaml).unwrap();

    // Load config
    let loaded_content = fs::read_to_string(&config_path).unwrap();
    let loaded_config: AppConfig = serde_yaml::from_str(&loaded_content).unwrap();

    assert_eq!(loaded_config.recent_paths.len(), 1);
    assert_eq!(loaded_config.recent_paths[0], PathBuf::from("/test/path"));
    assert_eq!(
        loaded_config.dotfiles_repo,
        Some("https://github.com/test/dotfiles".to_string())
    );
}

#[test]
fn test_comprehensive_env_var_workflow() {
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

    // Test adding environment variables for each context
    let contexts = vec![
        ("all", "ALL_TEST"),
        ("up", "UP_TEST"),
        ("exec", "EXEC_TEST"),
    ];

    for (context, var_name) in contexts {
        let output = Command::new(&devcon_binary)
            .args([
                "--config-file",
                temp_dir.path().join("config.yaml").to_str().unwrap(),
                "config",
                "envs",
                "add",
                var_name,
                "test_value",
                context,
            ])
            .output()
            .expect("Failed to execute devcon config envs add");

        assert!(
            output.status.success(),
            "Failed to add env var for context {}",
            context
        );
    }

    // Test listing all environment variables
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

    // Should contain all added variables
    assert!(output_text.contains("ALL_TEST"));
    assert!(output_text.contains("UP_TEST"));
    assert!(output_text.contains("EXEC_TEST"));

    // Should contain context information
    assert!(output_text.contains("All"));
    assert!(output_text.contains("Up"));
    assert!(output_text.contains("Exec"));

    // Test removing specific environment variables
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

    // Verify removal
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

    // Should have one less variable
    let var_count = output_text.matches("test_value").count();
    assert_eq!(var_count, 2);
}

#[test]
fn test_features_with_complex_json() {
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

    // Test adding features with complex JSON configurations
    let features = vec![
        (
            "ghcr.io/devcontainers/features/docker-in-docker:2",
            r#"{"version": "20.10", "moby": true}"#,
        ),
        (
            "ghcr.io/devcontainers/features/node:1",
            r#"{"version": "18", "nodeGypDependencies": true}"#,
        ),
        (
            "ghcr.io/devcontainers/features/python:1",
            r#"{"version": "3.11", "installTools": true}"#,
        ),
    ];

    for (feature, config) in features {
        let output = Command::new(&devcon_binary)
            .args([
                "--config-file",
                temp_dir.path().join("config.yaml").to_str().unwrap(),
                "config",
                "features",
                "add",
                feature,
                config,
            ])
            .output()
            .expect("Failed to execute devcon config features add");

        assert!(
            output.status.success(),
            "Failed to add feature: {}",
            feature
        );
    }

    // Test listing features
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

    // Should contain all added features
    assert!(output_text.contains("docker-in-docker"));
    assert!(output_text.contains("node"));
    assert!(output_text.contains("python"));

    // Should contain configuration values
    assert!(output_text.contains("20.10"));
    assert!(output_text.contains("18"));
    assert!(output_text.contains("3.11"));
}

#[test]
fn test_socket_path_generation() {
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

    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "socket",
            "--show-path",
        ])
        .output()
        .expect("Failed to execute devcon socket --show-path");

    assert!(output.status.success());
    let binding = String::from_utf8(output.stdout).unwrap();
    let socket_path = binding.trim();

    assert!(socket_path.ends_with("devcon.sock"));
}

#[test]
fn test_config_persistence() {
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

    // Add configuration
    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "dotfiles",
            "set",
            "https://github.com/test/dotfiles",
        ])
        .output()
        .expect("Failed to execute devcon config dotfiles set");

    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "add",
            "test-feature",
            "test-value",
        ])
        .output()
        .expect("Failed to execute devcon config features add");

    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "TEST_VAR",
            "test_value",
            "all",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");

    // Verify configuration persists across different command invocations
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
    assert!(output_text.contains("https://github.com/test/dotfiles"));

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
    assert!(output_text.contains("test-feature"));

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
    assert!(output_text.contains("TEST_VAR"));
}

#[test]
fn test_error_handling_edge_cases() {
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

    // Test removing non-existent environment variable
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

    // Test removing non-existent feature
    let output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "remove",
            "non-existent-feature",
        ])
        .output()
        .expect("Failed to execute devcon config features remove");

    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8(output.stderr).unwrap()
    );

    // Test adding environment variable with empty name
    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "envs",
            "add",
            "",
            "value",
            "all",
        ])
        .output()
        .expect("Failed to execute devcon config envs add");

    // Should handle gracefully (might succeed or fail depending on implementation)
    // The important thing is it doesn't crash

    // Test adding feature with empty name
    let _output = Command::new(&devcon_binary)
        .args([
            "--config-file",
            temp_dir.path().join("config.yaml").to_str().unwrap(),
            "config",
            "features",
            "add",
            "",
            "value",
        ])
        .output()
        .expect("Failed to execute devcon config features add");

    // Should handle gracefully
}
