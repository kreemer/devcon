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

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::bail;
use indicatif::ProgressBar;
use tracing::{info, trace};

use super::ContainerRuntime;

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
        let mut child = Command::new("container")
            .arg("build")
            .arg("-f")
            .arg(dockerfile_path)
            .arg("-t")
            .arg(image_tag)
            .arg(context_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        println!("Building Image..");
        let bar = ProgressBar::new_spinner();
        bar.enable_steady_tick(Duration::from_millis(150));

        // Stream stdout in a separate thread
        let stdout_thread = stdout.map(|stdout| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    trace!("{}", line);
                }
            })
        });

        // Stream stderr in main thread
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                trace!("{}", line);
            }
        }

        // Wait for stdout thread to complete
        if let Some(handle) = stdout_thread {
            let _ = handle.join();
        }

        let result = child.wait()?;

        bar.finish_with_message("Building image complete");
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
    ) -> anyhow::Result<Box<dyn super::ContainerHandle>> {
        let mut cmd = Command::new("container");
        cmd.arg("run")
            .arg("--rm")
            .arg("-d")
            .arg("-v")
            .arg(volume_mount)
            .arg("-l")
            .arg(label);

        // Add environment variables
        for env_var in env_vars {
            cmd.arg("-e").arg(env_var);
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

    fn tail_file(
        &self,
        container_handle: &dyn super::ContainerHandle,
        file_path: &str,
    ) -> anyhow::Result<std::process::Child> {
        let child = Command::new("container")
            .arg("exec")
            .arg("-i")
            .arg(container_handle.id())
            .arg("tail")
            .arg("-f")
            .arg(file_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(child)
    }
}
