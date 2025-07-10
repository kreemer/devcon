use std::path::PathBuf;
use std::process::Command;

use crate::config::AppConfig;

pub fn exec_devcontainer(path: &PathBuf, config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting devcontainer for: {}", path.display());

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

    // Add dotfiles repository if configured
    if let Some(ref dotfiles_repo) = config.dotfiles_repo {
        cmd.arg("--dotfiles-repository").arg(dotfiles_repo);
    }

    // Add additional features if configured
    for (feature, value) in &config.additional_features {
        cmd.arg("--additional-features")
           .arg(format!("{}={}", feature, value));
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to start devcontainer: {}", error).into());
    }

    println!("Successfully started devcontainer!");
    println!("Output: {}", String::from_utf8_lossy(&output.stdout));
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
    fn test_exec_devcontainer_no_devcontainer_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = AppConfig::default();
        let result = exec_devcontainer(&temp_dir.path().to_path_buf(), &config);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No .devcontainer directory or devcontainer.json found")
        );
    }

    #[test]
    fn test_exec_devcontainer_with_devcontainer_dir() {
        let temp_dir = TempDir::new().unwrap();
        let devcontainer_path = temp_dir.path().join(".devcontainer");
        fs::create_dir(&devcontainer_path).unwrap();
        let config = AppConfig::default();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = exec_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can't easily test the actual command execution without devcontainer CLI installed
        // but we can test that it doesn't fail due to missing .devcontainer directory
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_exec_devcontainer_with_devcontainer_json() {
        let temp_dir = TempDir::new().unwrap();
        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();
        let config = AppConfig::default();

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = exec_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_exec_devcontainer_with_dotfiles() {
        let temp_dir = TempDir::new().unwrap();
        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();
        
        let mut config = AppConfig::default();
        config.dotfiles_repo = Some("https://github.com/user/dotfiles".to_string());

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = exec_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }

    #[test]
    fn test_exec_devcontainer_with_additional_features() {
        let temp_dir = TempDir::new().unwrap();
        let devcontainer_file = temp_dir.path().join("devcontainer.json");
        fs::write(&devcontainer_file, "{}").unwrap();
        
        let mut config = AppConfig::default();
        config.additional_features.insert("ghcr.io/devcontainers/features/github-cli:1".to_string(), "latest".to_string());
        config.additional_features.insert("ghcr.io/devcontainers/features/docker-in-docker:2".to_string(), "20.10".to_string());

        // This test will fail if devcontainer CLI is not installed, which is expected
        let result = exec_devcontainer(&temp_dir.path().to_path_buf(), &config);

        // We can test that it doesn't fail due to missing devcontainer config
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory or devcontainer.json found"));
        }
    }
}
