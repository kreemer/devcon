#[cfg(test)]
mod tests {
    use assert_cmd::cargo::cargo_bin_cmd;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_build() {
        let temp_dir = tempfile::tempdir().unwrap();
        let container_content = r#"
        {
            "name": "devcontainer",
            "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
            "features": {
               "ghcr.io/devcontainers/features/node": {}
            }
        }
        "#;

        let devcontainer_path = temp_dir.path().join(".devcontainer");
        std::fs::create_dir_all(&devcontainer_path).unwrap();
        std::fs::write(
            devcontainer_path.join("devcontainer.json"),
            container_content,
        )
        .unwrap();

        let mut cmd = cargo_bin_cmd!("devcon");
        let result = cmd
            .arg("build")
            .arg(temp_dir.path().to_str().unwrap())
            .output();
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.status.success(),
            "Build command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_build_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let container_content = r#"
        {
            "name": "Test Devcontainer",
            "image": "mcr.microsoft.com/devcontainers/base:ubuntu",
            "features": {
               "ghcr.io/devcontainers/features/node": {}
            }
        }
        "#;

        let devcontainer_path = temp_dir.path().join(".devcontainer");
        std::fs::create_dir_all(&devcontainer_path).unwrap();
        std::fs::write(
            devcontainer_path.join("devcontainer.json"),
            container_content,
        )
        .unwrap();

        let mut cmd = cargo_bin_cmd!("devcon");
        let result = cmd
            .arg("build")
            .arg(temp_dir.path().to_str().unwrap())
            .output();
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.status.success(),
            "Build command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
