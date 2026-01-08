use std::{
    fs::{self, File},
    path::PathBuf,
    process::Command,
};

use anyhow::bail;
use tempfile::TempDir;

use crate::{devcontainer::Devcontainer, driver::process_features};

pub fn build(devcontainer: &Devcontainer) -> anyhow::Result<()> {
    let directory = TempDir::new()?;

    let processed_features = process_features(&devcontainer.features, &directory)?;
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
        devcontainer.image, feature_install
    );

    fs::write(&dockerfile, contents)?;

    let result = Command::new("container")
        .arg("build")
        .arg("-f")
        .arg(&dockerfile)
        .arg("-t")
        .arg(format!("devcon-{}", devcontainer.get_computed_name()))
        .arg(directory.path())
        .status();

    if result.is_err() {
        bail!("test")
    }

    directory.close()?;
    Ok(())
}

pub fn start(devcontainer: &Devcontainer, path: PathBuf) -> anyhow::Result<()> {
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
        .arg(format!("devcon-{}", devcontainer.get_computed_name()))
        .status();

    if result.is_err() {
        bail!("test")
    }
    Ok(())
}
