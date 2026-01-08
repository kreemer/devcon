use std::path::PathBuf;

use crate::{devcontainer::Devcontainer, driver::container::ContainerDriver};

pub fn handle_build_command(path: PathBuf) -> anyhow::Result<()> {
    let devcontainer = Devcontainer::try_from(path)?;
    let driver = ContainerDriver::new(&devcontainer);
    driver.build()?;

    Ok(())
}

pub fn handle_start_command(path: PathBuf) -> anyhow::Result<()> {
    let devcontainer = Devcontainer::try_from(path.clone())?;
    let canonical_path = std::fs::canonicalize(&path)?;
    let driver = ContainerDriver::new(&devcontainer);
    driver.start(canonical_path)?;

    Ok(())
}

pub fn handle_shell_command(_path: Option<&PathBuf>, _env: &[String]) -> anyhow::Result<()> {
    Ok(())
}
