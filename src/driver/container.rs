use std::{
    fs::{self, File},
    process::Command,
    vec,
};

use anyhow::bail;
use tempfile::TempDir;

use crate::devcontainer::{Devcontainer, Feature};

pub fn build(devcontainer: &Devcontainer) -> anyhow::Result<()> {
    let directory = TempDir::new()?;

    let processed_features = process_features(&devcontainer.features, &directory)?;
    let mut feature_install = String::new();
    for (feature, feature_path) in processed_features {
        feature_install.push_str(&format!(
            "COPY {}/* /opt/{}/ \n",
            feature_path,
            feature.get_name()?
        ));
        feature_install.push_str(&format!("RUN /opt/{}/install.sh \n", feature.get_name()?));
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
        .arg(format!("devcon-{}", devcontainer.name))
        .arg(directory.path())
        .status();

    if result.is_err() {
        bail!("test")
    }

    directory.close()?;
    Ok(())
}

fn process_features<'a>(
    features: &'a [Feature],
    directory: &'a TempDir,
) -> anyhow::Result<Vec<(&'a Feature, String)>> {
    let mut result: Vec<(&Feature, String)> = vec![];
    for feature in features {
        let feature_directory = directory.path().join(feature.get_name()?);
        fs::create_dir_all(&feature_directory)?;

        let token_url = format!(
            "https://{}/token?scope=repository:{}:pull",
            feature.get_registry()?,
            feature.get_repository()?
        );

        let response = reqwest::blocking::get(&token_url)?;
        if !response.status().is_success() {
            bail!("Failed to download feature: {}", feature.url);
        }
        let json: serde_json::Value = response.json()?;
        let token = json["token"].as_str().ok_or_else(|| {
            anyhow::anyhow!("Token not found in response for feature: {}", feature.url)
        })?;

        let manifest_url = format!(
            "https://{}/v2/{}/{}/manifests/{}",
            feature.get_registry()?,
            feature.get_repository()?,
            feature.get_name()?,
            feature.get_version()?
        );

        let manifest_response = reqwest::blocking::Client::new()
            .get(&manifest_url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.oci.image.manifest.v1+json")
            .send()?;

        if !manifest_response.status().is_success() {
            bail!("Failed to download manifest for feature: {}", feature.url);
        }
        let manifest_json: serde_json::Value = manifest_response.json()?;
        let manifest_str = serde_json::to_string(&manifest_json)?;
        let reader = std::io::Cursor::new(manifest_str);
        let _manifest = oci_spec::image::ImageManifest::from_reader(reader)?;
        let layer = _manifest.layers().first().ok_or_else(|| {
            anyhow::anyhow!("No layers found in manifest for feature: {}", feature.url)
        })?;

        let layer_url = format!(
            "https://{}/v2/{}/{}/blobs/{}",
            feature.get_registry()?,
            feature.get_repository()?,
            feature.get_name()?,
            layer.digest()
        );
        let layer_response = reqwest::blocking::Client::new()
            .get(&layer_url)
            .bearer_auth(token)
            .send()?;

        if !layer_response.status().is_success() {
            bail!("Failed to download layer for feature: {}", feature.url);
        }
        let _layer_bytes = layer_response.bytes()?;

        let feature_file = format!("feature-{}.tar.gz", feature.get_name()?);
        let feature_file_path = &feature_directory.join(&feature_file);
        fs::write(&feature_file_path, &_layer_bytes)?;

        let feature_archive = File::open(&feature_file_path)?;
        let mut archive = tar::Archive::new(flate2::write::GzDecoder::new(feature_archive));
        let extract_path = &feature_directory.join("extract");
        fs::create_dir_all(&extract_path)?;
        archive.unpack(&extract_path)?;
        let extracted_path = extract_path
            .to_str()
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to convert path to str for feature: {}", feature.url)
            })?
            .to_string();

        let relative_path = extracted_path
            .strip_prefix(directory.path().to_str().unwrap())
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to get relative path for feature: {}", feature.url)
            })?
            .trim_matches('/')
            .to_string();
        result.push((feature, relative_path));
    }
    Ok(result)
}

pub fn start(devcontainer: &Devcontainer, path: &str) -> anyhow::Result<()> {
    let result = Command::new("container")
        .arg("run")
        .arg("--rm")
        .arg("-d")
        .arg("-v")
        .arg(format!("{}:/workspaces/{}", path, "project"))
        .arg(format!("devcon-{}", devcontainer.name))
        .status();

    if result.is_err() {
        bail!("test")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_feature_download() {
        let directory = TempDir::new().unwrap();
        let features = vec![Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: serde_json::Value::Null,
        }];
        let response = process_features(&features, &directory);
        assert!(
            response.is_ok(),
            "Feature processing failed: {:?}",
            response
        );

        drop(directory);
    }

    #[test]
    fn test_handle_build_command() {
        let devcontainer = Devcontainer {
            name: "test-devcontainer".to_string(),
            image: "mcr.microsoft.com/devcontainers/base:ubuntu".to_string(),
            features: vec![Feature {
                url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
                options: serde_json::Value::Null,
            }],
        };
        let response = build(&devcontainer);
        assert!(response.is_ok(), "Build processing failed: {:?}", response);
    }
}
