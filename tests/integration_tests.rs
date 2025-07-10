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
