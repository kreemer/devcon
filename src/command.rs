use std::path::PathBuf;

use crate::devcontainer::Devcontainer;

pub fn handle_build_command(path: PathBuf) -> anyhow::Result<()> {
    let devcontainer = Devcontainer::try_from(path)?;
    crate::driver::container::build(&devcontainer)?;

    Ok(())
}

pub fn handle_start_command(path: PathBuf) -> anyhow::Result<()> {
    let devcontainer = Devcontainer::try_from(path.clone())?;
    let canonical_path = std::fs::canonicalize(&path)?;
    crate::driver::container::start(&devcontainer, canonical_path)?;

    Ok(())
}

pub fn handle_shell_command(_path: Option<&PathBuf>, _env: &[String]) -> anyhow::Result<()> {
    Ok(())
}
