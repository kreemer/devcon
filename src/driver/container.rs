use std::{
    fs::{self, File},
    path::PathBuf,
    process::Command,
};

use anyhow::bail;
use tempfile::TempDir;

use crate::{devcontainer::Devcontainer, driver::process_features};

pub struct ContainerDriver<'a> {
    devcontainer: &'a Devcontainer,
}

impl<'a> ContainerDriver<'a> {
    pub fn new(devcontainer: &'a Devcontainer) -> Self {
        Self { devcontainer }
    }

    pub fn build(&self) -> anyhow::Result<()> {
        let directory = TempDir::new()?;

        let processed_features = process_features(&self.devcontainer.features, &directory)?;
        let mut feature_install = String::new();

        for (feature, feature_path) in processed_features {
            let feature_name = match &feature.source {
                crate::devcontainer::FeatureSource::Registry {
                    registry_type: _,
                    registry,
                } => &registry.name,
                crate::devcontainer::FeatureSource::Local { path } => {
                    &path.to_string_lossy().to_string()
                }
            };
            feature_install.push_str(&format!(
                "COPY {}/* /opt/{}/ \n",
                feature_path, feature_name
            ));
            feature_install.push_str(&format!("RUN /opt/{}/install.sh \n", feature_name));
        }

        let dockerfile = directory.path().join("Dockerfile");
        File::create(&dockerfile)?;

        let contents = format!(
            r#"
FROM {}
{}
CMD ["sleep", "infinity"]
    "#,
            self.devcontainer.image, feature_install
        );

        fs::write(&dockerfile, contents)?;

        let result = Command::new("container")
            .arg("build")
            .arg("-f")
            .arg(&dockerfile)
            .arg("-t")
            .arg(self.get_image_tag())
            .arg(directory.path())
            .status();

        if result.is_err() {
            bail!("Failed to build container image")
        }

        directory.close()?;
        Ok(())
    }

    pub fn start(&self, path: PathBuf) -> anyhow::Result<()> {
        let result = Command::new("container")
            .arg("run")
            .arg("--rm")
            .arg("-d")
            .arg("-v")
            .arg(format!(
                "{}:/workspaces/{}",
                path.to_string_lossy(),
                path.file_name().unwrap().to_string_lossy()
            ))
            .arg(self.get_image_tag())
            .status();

        if result.is_err() {
            bail!("Failed to start container")
        }

        Ok(())
    }

    fn get_image_tag(&self) -> String {
        format!("devcon-{}", self.devcontainer.get_computed_name())
    }
}
