use std::fs;
use std::path::PathBuf;

use anyhow::bail;
use serde::Deserialize;
use serde::de;

#[derive(Debug)]
pub struct Devcontainer {
    pub name: Option<String>,
    pub image: String,
    pub features: Vec<Feature>,
    #[allow(dead_code)]
    pub remote_user: Option<String>,
}

impl<'de> Deserialize<'de> for Devcontainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DevcontainerHelper {
            name: Option<String>,
            image: String,
            #[serde(default)]
            #[serde(deserialize_with = "deserialize_features_map")]
            features: Vec<(String, serde_json::Value)>,
            remote_user: Option<String>,
        }

        let helper = DevcontainerHelper::deserialize(deserializer)?;

        let features: Result<Vec<Feature>, D::Error> = helper
            .features
            .into_iter()
            .map(|(url, options)| parse_feature(&url, options))
            .collect();

        Ok(Devcontainer {
            name: helper.name,
            image: helper.image,
            features: features?,
            remote_user: helper.remote_user,
        })
    }
}

fn deserialize_features_map<'de, D>(
    deserializer: D,
) -> Result<Vec<(String, serde_json::Value)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FeaturesVisitor;

    impl<'de> serde::de::Visitor<'de> for FeaturesVisitor {
        type Value = Vec<(String, serde_json::Value)>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map of features")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut features = Vec::new();
            while let Some((key, value)) = map.next_entry()? {
                features.push((key, value));
            }
            Ok(features)
        }
    }

    deserializer.deserialize_map(FeaturesVisitor)
}

impl Devcontainer {
    pub fn get_computed_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| "default".to_string())
    }
}

#[derive(Debug)]
pub struct Feature {
    pub source: FeatureSource,
    #[allow(dead_code)]
    pub options: serde_json::Value,
}

#[derive(Debug)]
pub enum FeatureSource {
    Registry {
        registry_type: FeatureRegistryType,
        registry: FeatureRegistry,
    },
    Local {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub struct FeatureRegistry {
    pub owner: String,
    pub repository: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug)]
pub enum FeatureRegistryType {
    Ghcr,
}

fn parse_feature<E: de::Error>(url: &str, options: serde_json::Value) -> Result<Feature, E> {
    if !url.starts_with("ghcr.io") && url.contains(":") {
        return Err(de::Error::custom("Only ghcr.io features are supported"));
    }

    if url.starts_with("ghcr.io") {
        parse_registry_feature(url, options)
    } else {
        parse_local_feature(url, options)
    }
}

fn parse_local_feature<E: de::Error>(url: &str, options: serde_json::Value) -> Result<Feature, E> {
    let path = PathBuf::from(url);
    Ok(Feature {
        source: FeatureSource::Local { path },
        options,
    })
}

fn parse_registry_feature<E: de::Error>(
    url: &str,
    options: serde_json::Value,
) -> Result<Feature, E> {
    let owner = url
        .split("/")
        .nth(1)
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing owner information"))?;
    let repository = url
        .split("/")
        .nth(2)
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing repository information"))?;
    let name = url
        .split("/")
        .nth(3)
        .and_then(|s| s.split(":").next())
        .ok_or_else(|| de::Error::custom("Invalid feature URL, missing name information"))?;

    let version = url
        .split("/")
        .nth(3)
        .and_then(|s| s.split(":").nth(1))
        .unwrap_or("latest");

    Ok(Feature {
        source: FeatureSource::Registry {
            registry_type: FeatureRegistryType::Ghcr,
            registry: FeatureRegistry {
                owner: owner.to_string(),
                repository: repository.to_string(),
                name: name.to_string(),
                version: version.to_string(),
            },
        },
        options,
    })
}

impl TryFrom<PathBuf> for Devcontainer {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let final_path = path.join(".devcontainer").join("devcontainer.json");
        if fs::exists(&final_path).is_err() {
            bail!(
                "Devcontainer definition not found in {}",
                &final_path.to_string_lossy()
            )
        }

        let file_result = fs::read_to_string(&final_path);

        if file_result.is_err() {
            bail!(
                "Devcontainer definition cannot be read {}",
                &final_path.to_string_lossy()
            )
        }

        let result = Self::try_from(file_result.unwrap());
        if result.is_err() {
            bail!("Devcontainer content could not be parsed")
        }

        // Fix name of container if not present
        let mut result = result?;
        if result.name.is_none() {
            let name = fs::canonicalize(&path)?
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid path for devcontainer"))?
                .to_string_lossy()
                .to_string();
            result.name = Some(name);
        }

        Ok(result)
    }
}

impl TryFrom<String> for Devcontainer {
    type Error = serde_json::Error;

    fn try_from(content: String) -> std::result::Result<Self, Self::Error> {
        serde_json::from_str(&content)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_feature() {
        let feature = Feature {
            source: FeatureSource::Registry {
                registry_type: FeatureRegistryType::Ghcr,
                registry: FeatureRegistry {
                    owner: "devcontainers".to_string(),
                    repository: "features".to_string(),
                    name: "github-cli".to_string(),
                    version: "1".to_string(),
                },
            },
            options: serde_json::Value::Null,
        };

        assert!(feature.options.is_null());
        match feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_feature_parsing() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/github-cli:1": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 1);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_multiple_feature_parsing() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "ghcr.io/devcontainers/features/github-cli:1": {},
               "ghcr.io/devcontainers/features/node:2": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 2);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("github-cli", registry.name);
                assert_eq!("1", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
        let feature = &devcontainer.features[1];
        match &feature.source {
            FeatureSource::Registry {
                registry_type,
                registry,
            } => {
                match registry_type {
                    FeatureRegistryType::Ghcr => {}
                }
                assert_eq!("devcontainers", registry.owner);
                assert_eq!("features", registry.repository);
                assert_eq!("node", registry.name);
                assert_eq!("2", registry.version);
            }
            _ => assert!(false, "Feature source should be Registry"),
        }
    }

    #[test]
    fn test_local_feature() {
        let feature_json = r#"
        {
            "name": "test",
            "image": "ubuntu:20.04",
            "features": {
               "./devfeatures/myfeature": {}
            }
        }
        "#;

        let devcontainer: Devcontainer = serde_json::from_str(feature_json).unwrap();

        assert_eq!(devcontainer.name.as_deref(), Some("test"));
        assert_eq!(devcontainer.features.len(), 1);
        let feature = &devcontainer.features[0];
        match &feature.source {
            FeatureSource::Local { path } => {
                assert_eq!(PathBuf::from("./devfeatures/myfeature"), *path);
            }
            _ => assert!(false, "Feature source should be Local"),
        }
    }
}
