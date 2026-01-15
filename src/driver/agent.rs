use anyhow::{Context, Result};
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::TempDir;

/// Configuration for generating a devcontainer feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Feature ID (e.g., "my-feature")
    pub id: String,
    /// Feature version
    pub version: String,
    /// Feature name
    pub name: String,
    /// Feature description
    pub description: Option<String>,
    /// Install script content
    pub install_script: String,
    /// Additional options for the feature
    pub options: Option<serde_json::Value>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        let env = Environment::new();
        let template = env
            .template_from_str(
                r###"
#!/bin/bash

set -e
echo "Installing DevCon Agent..."

echo "Install protoc..."
PB_REL="https://github.com/protocolbuffers/protobuf/releases"
curl -LO $PB_REL/download/v30.2/protoc-30.2-linux-x86_64.zip
unzip protoc-30.2-linux-x86_64.zip -d $HOME/.local
export PATH="$PATH:$HOME/.local/bin"

if [ ! -f $HOME/.cargo/env ]; then
    echo "Installing Rust toolchain..."
    curl https://sh.rustup.rs -sSf | sh -s -- -y 
fi
. "$HOME/.cargo/env"  

git clone https://github.com/kreemer/devcon.git /tmp/devcon
cd /tmp/devcon
git checkout support_reference
cargo b --release --workspace --bin devcon-agent
mv target/release/devcon-agent /usr/local/bin/devcon-agent
rm -rf /tmp/devcon

mkdir -p /tmp/devcon-sockets
chmod 777 /tmp/devcon-sockets
echo "DevCon Agent installed successfully."
"###,
            )
            .expect("Failed to create template");

        let contents = template
            .render(minijinja::context! {})
            .expect("Could not create install script");

        Self {
            id: "devcon-agent".to_string(),
            version: "1.0.0".to_string(),
            name: "DevCon Agent".to_string(),
            description: Some("DevCon Agent for managing devcontainer features".to_string()),
            install_script: contents,
            options: None,
        }
    }
}

/// Agent for generating devcontainer features
pub struct Agent {
    config: AgentConfig,
}

impl Agent {
    /// Create a new Agent with the given configuration
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    /// Generate the devcontainer feature in a temporary directory
    pub fn generate(&mut self) -> Result<PathBuf> {
        // Create temporary directory
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let feature_dir = temp_dir.keep().join(&self.config.id);

        // Create feature directory
        std::fs::create_dir_all(&feature_dir).context("Failed to create feature directory")?;

        // Generate devcontainer-feature.json
        self.generate_feature_json(&feature_dir)?;

        // Generate install.sh
        self.generate_install_script(&feature_dir)?;

        let path = feature_dir.clone();

        Ok(path)
    }

    /// Generate the devcontainer-feature.json file
    fn generate_feature_json(&self, feature_dir: &PathBuf) -> Result<()> {
        let mut feature_json = serde_json::json!({
            "id": self.config.id,
            "version": self.config.version,
            "name": self.config.name,
            "mounts": [
                {
                    "type": "bind",
                    "source": "/tmp/devcon-sockets",
                    "target": "/tmp/devcon-sockets"
                }
            ]
        });

        if let Some(desc) = &self.config.description {
            feature_json["description"] = serde_json::Value::String(desc.clone());
        }

        if let Some(options) = &self.config.options {
            feature_json["options"] = options.clone();
        }

        let json_path = feature_dir.join("devcontainer-feature.json");
        let json_content = serde_json::to_string_pretty(&feature_json)
            .context("Failed to serialize feature JSON")?;

        std::fs::write(&json_path, json_content)
            .context("Failed to write devcontainer-feature.json")?;

        Ok(())
    }

    /// Generate the install.sh script
    fn generate_install_script(&self, feature_dir: &PathBuf) -> Result<()> {
        let install_path = feature_dir.join("install.sh");
        std::fs::write(&install_path, &self.config.install_script)
            .context("Failed to write install.sh")?;

        // Make install.sh executable on Unix systems
        #[cfg(unix)]
        {
            let mut perms = std::fs::metadata(&install_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&install_path, perms)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_generation() {
        let config = AgentConfig {
            id: "test-feature".to_string(),
            version: "1.0.0".to_string(),
            name: "Test Feature".to_string(),
            description: Some("A test feature".to_string()),
            install_script: "#!/bin/bash\necho 'Installing...'".to_string(),
            options: None,
        };

        let mut agent = Agent::new(config);
        let path = agent.generate().expect("Failed to generate feature");

        assert!(path.exists());
        assert!(path.join("devcontainer-feature.json").exists());
        assert!(path.join("install.sh").exists());
    }
}
