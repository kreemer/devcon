use std::path::PathBuf;
use std::process::Command;

pub fn exec_devcontainer(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Opening devcontainer for: {}", path.display());

    // Check if the path has a .devcontainer directory
    let devcontainer_path = path.join(".devcontainer");
    if !devcontainer_path.exists() {
        return Err(format!("No .devcontainer directory found in {}", path.display()).into());
    }

    // Use VS Code command to open in devcontainer
    let output = Command::new("code")
        .arg("--folder-uri")
        .arg(format!(
            "vscode-remote://dev-container+{}",
            urlencoding::encode(&path.to_string_lossy())
        ))
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to open devcontainer: {}", error).into());
    }

    println!("Successfully opened devcontainer!");
    Ok(())
}

pub fn check_devcontainer_cli() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("code").arg("--version").output();

    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => Err("VS Code is installed but not working properly".into()),
        Err(_) => Err("VS Code is not installed or not in PATH. Please install VS Code and ensure it's available in your PATH.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_exec_devcontainer_no_devcontainer_dir() {
        let temp_dir = TempDir::new().unwrap();
        let result = exec_devcontainer(&temp_dir.path().to_path_buf());

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No .devcontainer directory found")
        );
    }

    #[test]
    fn test_exec_devcontainer_with_devcontainer_dir() {
        let temp_dir = TempDir::new().unwrap();
        let devcontainer_path = temp_dir.path().join(".devcontainer");
        fs::create_dir(&devcontainer_path).unwrap();

        // This test will fail if VS Code is not installed, which is expected
        let result = exec_devcontainer(&temp_dir.path().to_path_buf());

        // We can't easily test the actual command execution without VS Code installed
        // but we can test that it doesn't fail due to missing .devcontainer directory
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("No .devcontainer directory found"));
        }
    }
}
