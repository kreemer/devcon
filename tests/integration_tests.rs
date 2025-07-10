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
use std::env;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_integration_config_workflow() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", temp_dir.path()) };

    // Test creating a new config manager
    let config_manager = ConfigManager::new().unwrap();

    // Test loading or creating config
    let config = config_manager.load_or_create_config().unwrap();
    assert!(config.recent_paths.is_empty());

    // Test adding a path
    let test_path = temp_dir.path().join("test_project");
    fs::create_dir(&test_path).unwrap();

    let updated_config = config_manager
        .add_recent_path(config, test_path.clone())
        .unwrap();
    assert_eq!(updated_config.recent_paths.len(), 1);

    // Test that config persists
    let reloaded_config = config_manager.load_config().unwrap();
    assert_eq!(reloaded_config.recent_paths.len(), 1);
    assert_eq!(
        reloaded_config.recent_paths[0],
        test_path.canonicalize().unwrap()
    );
}

#[test]
fn test_integration_multiple_paths() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", temp_dir.path()) };

    let config_manager = ConfigManager::new().unwrap();
    let mut config = config_manager.load_or_create_config().unwrap();

    // Add multiple paths
    for i in 0..5 {
        let test_path = temp_dir.path().join(format!("project_{}", i));
        fs::create_dir(&test_path).unwrap();
        config = config_manager.add_recent_path(config, test_path).unwrap();
    }

    assert_eq!(config.recent_paths.len(), 5);

    // Test that the most recent is first
    let last_path = temp_dir.path().join("project_4").canonicalize().unwrap();
    assert_eq!(config.recent_paths[0], last_path);
}
