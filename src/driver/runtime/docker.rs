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

//! # Docker Runtime
//!
//! Implementation of ContainerRuntime trait for Docker CLI.

use std::path::Path;
use std::process::Command;

use anyhow::bail;

use super::ContainerRuntime;

/// Docker CLI runtime implementation.
pub struct DockerRuntime;

impl DockerRuntime {
    pub fn new() -> Self {
        Self
    }
}

/// Handle for a Docker container instance.
pub struct DockerContainerHandle {
    id: String,
}

impl super::ContainerHandle for DockerContainerHandle {
    fn id(&self) -> &str {
        &self.id
    }
}

impl ContainerRuntime for DockerRuntime {
    fn build(
        &self,
        dockerfile_path: &Path,
        context_path: &Path,
        image_tag: &str,
    ) -> anyhow::Result<()> {
        let result = Command::new("docker")
            .arg("build")
            .arg("-f")
            .arg(dockerfile_path)
            .arg("-t")
            .arg(image_tag)
            .arg(context_path)
            .status()?;

        if result.code() != Some(0) {
            bail!("Docker build command failed")
        }

        Ok(())
    }

    fn run(
        &self,
        image_tag: &str,
        volume_mount: &str,
        label: &str,
        env_vars: &[String],
    ) -> anyhow::Result<Box<dyn super::ContainerHandle>> {
        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("-d")
            .arg("-v")
            .arg(volume_mount)
            .arg("--label")
            .arg(label);

        // Add environment variables
        for env_var in env_vars {
            cmd.arg("-e").arg(env_var);
        }

        cmd.arg(image_tag);

        let result = cmd.output()?;

        if result.status.code() != Some(0) {
            bail!("Docker run command failed")
        }

        Ok(Box::new(DockerContainerHandle {
            id: String::from_utf8_lossy(&result.stdout).to_string(),
        }))
    }

    fn exec(
        &self,
        container_handle: &dyn super::ContainerHandle,
        command: Vec<&str>,
        env_vars: &[String],
    ) -> anyhow::Result<()> {
        let mut cmd = Command::new("docker");
        cmd.arg("exec").arg("-it");

        for env_var in env_vars {
            cmd.arg("-e").arg(env_var);
        }

        let result = cmd.arg(container_handle.id()).args(command).status()?;

        if result.code() != Some(0) {
            bail!("Docker exec command failed")
        }

        Ok(())
    }

    fn list(&self) -> anyhow::Result<Vec<(String, Box<dyn super::ContainerHandle>)>> {
        let output = Command::new("docker")
            .arg("ps")
            .arg("--filter")
            .arg("label=devcon")
            .arg("--format")
            .arg("{{json .}}")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut result: Vec<(String, Box<dyn super::ContainerHandle>)> = Vec::new();

        // Docker outputs one JSON object per line, not an array
        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let container: serde_json::Value = serde_json::from_str(line)?;

            // Parse labels to find devcon label value
            let labels = container["Labels"].as_str().unwrap_or_default();
            let mut devcon_name = String::new();

            // Labels format: "key1=value1,key2=value2"
            for label_pair in labels.split(',') {
                if let Some((key, value)) = label_pair.split_once('=')
                    && key == "devcon"
                {
                    devcon_name = value.to_string();
                    break;
                }
            }

            let id = container["ID"].as_str().unwrap_or_default().to_string();

            if !devcon_name.is_empty() {
                let handle = DockerContainerHandle { id: id.clone() };
                result.push((devcon_name, Box::new(handle)));
            }
        }

        Ok(result)
    }
}
