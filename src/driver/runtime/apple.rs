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

//! # Apple Container Runtime
//!
//! Implementation of ContainerRuntime trait for Apple's `container` CLI.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::bail;

use crate::driver::runtime::RuntimeParameters;

use super::{ContainerRuntime, stream_build_output};

/// Apple's container CLI runtime implementation.
pub struct AppleRuntime;

/// Handle for an Apple container instance.
pub struct AppleContainerHandle {
    id: String,
}

impl super::ContainerHandle for AppleContainerHandle {
    fn id(&self) -> &str {
        &self.id
    }
}

impl AppleRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl ContainerRuntime for AppleRuntime {
    fn build(
        &self,
        dockerfile_path: &Path,
        context_path: &Path,
        image_tag: &str,
    ) -> anyhow::Result<()> {
        let child = Command::new("container")
            .arg("build")
            .arg("-f")
            .arg(dockerfile_path)
            .arg("-t")
            .arg(image_tag)
            .arg(context_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let result = stream_build_output(child)?;

        if !result.success() {
            bail!("Container build command failed")
        }

        Ok(())
    }

    fn run(
        &self,
        image_tag: &str,
        volume_mount: &str,
        label: &str,
        env_vars: &[String],
        runtime_parameters: RuntimeParameters,
    ) -> anyhow::Result<Box<dyn super::ContainerHandle>> {
        let mut cmd = Command::new("container");
        cmd.arg("run")
            .arg("--rm")
            .arg("-d")
            .arg("-v")
            .arg(volume_mount)
            .arg("-l")
            .arg(label);

        // Add privileged flag if required
        if runtime_parameters.requires_privileged {
            cmd.arg("--privileged");
        }

        // Add environment variables
        for env_var in env_vars {
            cmd.arg("-e").arg(env_var);
        }

        // Add additional mounts from features and devcontainer config
        for mount in runtime_parameters.additional_mounts {
            match mount {
                crate::devcontainer::Mount::String(mount_str) => {
                    cmd.arg("-v").arg(mount_str);
                }
                crate::devcontainer::Mount::Structured(structured) => {
                    let mount_arg = match &structured.mount_type {
                        crate::devcontainer::MountType::Bind => {
                            if let Some(source) = &structured.source {
                                format!("type=bind,source={},target={}", source, structured.target)
                            } else {
                                continue; // Skip bind mounts without source
                            }
                        }
                        crate::devcontainer::MountType::Volume => {
                            if let Some(source) = &structured.source {
                                format!(
                                    "type=volume,source={},target={}",
                                    source, structured.target
                                )
                            } else {
                                format!("type=volume,target={}", structured.target)
                            }
                        }
                    };
                    cmd.arg("--mount").arg(mount_arg);
                }
            }
        }

        // Add port forwards
        for port in runtime_parameters.ports {
            cmd.arg("-p").arg(port.to_string());
        }

        cmd.arg(image_tag);

        let result = cmd.output()?;

        if result.status.code() != Some(0) {
            bail!("Container start command failed")
        }

        Ok(Box::new(AppleContainerHandle {
            id: String::from_utf8_lossy(&result.stdout).to_string(),
        }))
    }

    fn exec(
        &self,
        container_handle: &dyn super::ContainerHandle,
        command: Vec<&str>,
        env_vars: &[String],
    ) -> anyhow::Result<()> {
        let mut cmd = Command::new("container");
        cmd.arg("exec").arg("-it");

        for env_var in env_vars {
            cmd.arg("-e").arg(env_var);
        }

        let result = cmd.arg(container_handle.id()).args(command).status()?;

        if result.code() != Some(0) {
            bail!("Container exec command failed")
        }

        Ok(())
    }

    fn list(&self) -> anyhow::Result<Vec<(String, Box<dyn super::ContainerHandle>)>> {
        let output = Command::new("container")
            .arg("list")
            .arg("--format")
            .arg("json")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let containers: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;

        let result: Vec<(String, Box<dyn super::ContainerHandle>)> = containers
            .iter()
            .filter_map(|container| {
                let project_name = container["configuration"]["labels"]["devcon.project"]
                    .as_str()
                    .unwrap_or_default();

                if project_name.is_empty() {
                    return None;
                }

                let id = container["configuration"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                let container_name = format!("devcon.{}", project_name);
                let handle = AppleContainerHandle { id };
                Some((
                    container_name,
                    Box::new(handle) as Box<dyn super::ContainerHandle>,
                ))
            })
            .collect();

        Ok(result)
    }

    fn images(&self) -> anyhow::Result<Vec<String>> {
        let output = Command::new("container")
            .arg("images")
            .arg("--format")
            .arg("json")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let images: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;

        let result: Vec<String> = images
            .iter()
            .filter_map(|image| {
                let name = &image["reference"];
                if name.is_null() {
                    return None;
                }

                if name.as_str().unwrap_or_default().starts_with("devcon") {
                    Some(name.as_str().unwrap_or_default().to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(result)
    }
}
