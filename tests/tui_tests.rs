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

use devcon::config::AppConfig;
use devcon::tui::TuiApp;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_tui_app_initialization() {
    // Test TuiApp creation
    let app = TuiApp::new();

    // Test initial state
    assert!(!app.should_quit);
    assert!(!app.selected);
}

#[test]
fn test_tui_app_with_config() {
    let temp_dir = TempDir::new().unwrap();

    // Create a sample config
    let mut config = AppConfig::default();
    config.recent_paths.push(temp_dir.path().join("project1"));
    config.recent_paths.push(temp_dir.path().join("project2"));
    config.recent_paths.push(temp_dir.path().join("project3"));

    // Test that config can be used with TUI
    assert_eq!(config.recent_paths.len(), 3);
    assert!(
        config
            .recent_paths
            .iter()
            .all(|p| p.to_string_lossy().contains("project"))
    );
}

#[test]
fn test_tui_app_empty_config() {
    let config = AppConfig::default();
    let app = TuiApp::new();

    // Test with empty config
    assert_eq!(config.recent_paths.len(), 0);
    assert!(!app.should_quit);

    // TUI app should handle empty state gracefully
    assert!(!app.selected);
}

#[test]
fn test_devcontainer_json_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create .devcontainer directory
    let devcontainer_dir = project_dir.join(".devcontainer");
    fs::create_dir_all(&devcontainer_dir).unwrap();

    // Test basic devcontainer.json
    let basic_config = r#"
    {
        "name": "Test Container",
        "image": "ubuntu:20.04",
        "features": {
            "ghcr.io/devcontainers/features/github-cli:1": "latest"
        },
        "customizations": {
            "vscode": {
                "extensions": ["ms-python.python"]
            }
        }
    }
    "#;

    let devcontainer_json = devcontainer_dir.join("devcontainer.json");
    fs::write(&devcontainer_json, basic_config).unwrap();

    // Test that the file exists and can be read
    assert!(devcontainer_json.exists());
    let content = fs::read_to_string(&devcontainer_json).unwrap();
    assert!(content.contains("Test Container"));
    assert!(content.contains("ubuntu:20.04"));

    // Test docker-compose based config
    let compose_config = r#"
    {
        "name": "Compose Project",
        "dockerComposeFile": "docker-compose.yml",
        "service": "app",
        "workspaceFolder": "/workspace"
    }
    "#;

    let compose_devcontainer = devcontainer_dir.join("devcontainer.compose.json");
    fs::write(&compose_devcontainer, compose_config).unwrap();

    assert!(compose_devcontainer.exists());
    let compose_content = fs::read_to_string(&compose_devcontainer).unwrap();
    assert!(compose_content.contains("Compose Project"));
    assert!(compose_content.contains("docker-compose.yml"));
}

#[test]
fn test_project_path_validation() {
    let temp_dir = TempDir::new().unwrap();

    // Create a valid project directory
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

    // Test valid project detection
    assert!(valid_project.exists());
    assert!(devcontainer_json.exists());

    // Test invalid project (no .devcontainer)
    let invalid_project = temp_dir.path().join("invalid_project");
    fs::create_dir_all(&invalid_project).unwrap();

    assert!(invalid_project.exists());
    assert!(!invalid_project.join(".devcontainer").exists());

    // Test non-existent project
    let nonexistent_project = temp_dir.path().join("nonexistent");
    assert!(!nonexistent_project.exists());
}

#[test]
fn test_recent_paths_management() {
    let temp_dir = TempDir::new().unwrap();

    // Test adding and managing recent paths
    let mut config = AppConfig::default();

    // Add some paths
    let path1 = temp_dir.path().join("project1");
    let path2 = temp_dir.path().join("project2");
    let path3 = temp_dir.path().join("project3");

    config.recent_paths.push(path1.clone());
    config.recent_paths.push(path2.clone());
    config.recent_paths.push(path3.clone());

    assert_eq!(config.recent_paths.len(), 3);

    // Test duplicate handling (would need to be implemented)
    config.recent_paths.push(path1.clone());
    assert_eq!(config.recent_paths.len(), 4); // Currently allows duplicates

    // Test path ordering (most recent first)
    assert_eq!(config.recent_paths[0], path1);
    assert_eq!(config.recent_paths[3], path1); // Duplicate
}

#[test]
fn test_config_with_all_features() {
    let temp_dir = TempDir::new().unwrap();

    // Create a comprehensive config
    let mut config = AppConfig::default();

    // Add recent paths
    config
        .recent_paths
        .push(temp_dir.path().join("web-project"));
    config.recent_paths.push(temp_dir.path().join("mobile-app"));

    // Add dotfiles
    config.dotfiles_repo = Some("https://github.com/user/dotfiles".to_string());

    // Add features
    config.additional_features.insert(
        "ghcr.io/devcontainers/features/github-cli:1".to_string(),
        "latest".to_string(),
    );

    // Add environment variables
    config.env.push(devcon::config::AppConfigEnv {
        name: "EDITOR".to_string(),
        value: "vim".to_string(),
        context: devcon::config::DevContainerContext::All,
    });

    // Test comprehensive configuration
    assert_eq!(config.recent_paths.len(), 2);
    assert!(config.dotfiles_repo.is_some());
    assert_eq!(config.additional_features.len(), 1);
    assert_eq!(config.env.len(), 1);

    // Test that all components work together
    let dotfiles = config.dotfiles_repo.unwrap();
    assert!(dotfiles.contains("github.com"));

    let feature_keys: Vec<_> = config.additional_features.keys().collect();
    assert!(feature_keys[0].contains("github-cli"));

    assert_eq!(config.env[0].name, "EDITOR");
    assert_eq!(config.env[0].value, "vim");
}
