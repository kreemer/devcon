mod config;
mod devcontainer;
mod tui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use config::ConfigManager;
use devcontainer::{check_devcontainer_cli, exec_devcontainer};
use tui::TuiApp;

#[derive(Parser)]
#[command(
    name = "devcon",
    about = "A TUI application for managing and launching development containers",
    long_about = "DevCon helps you manage your development containers by keeping track of recent projects and providing a convenient interface to launch them in VS Code.",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a development container for the specified path
    #[command(about = "Open a development container in VS Code")]
    Open {
        /// Path to the project directory containing .devcontainer configuration
        #[arg(
            help = "Path to the project directory. If not provided, uses current directory.",
            value_name = "PATH"
        )]
        path: Option<PathBuf>,
    },
    /// Check if the required tools are available
    #[command(about = "Check if VS Code and devcontainer CLI are properly installed")]
    Check,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config_manager = ConfigManager::new()?;

    match &cli.command {
        Some(Commands::Open { path }) => {
            handle_open_command(&config_manager, path.as_ref())?;
        }
        Some(Commands::Check) => {
            handle_check_command()?;
        }
        None => {
            handle_tui_mode(&config_manager)?;
        }
    }

    Ok(())
}

fn handle_open_command(
    config_manager: &ConfigManager,
    path: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let open_path = path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    if !open_path.exists() {
        return Err(format!(
            "The specified path '{}' does not exist.",
            open_path.display()
        )
        .into());
    }

    // Convert to absolute path
    let open_path = open_path.canonicalize()?;

    // Load current config and add the new path
    let config = config_manager.load_or_create_config()?;
    let updated_config = config_manager.add_recent_path(config, open_path.clone())?;

    // Execute devcontainer
    exec_devcontainer(&updated_config.recent_paths[0])?;

    Ok(())
}

fn handle_check_command() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking system requirements...");

    match check_devcontainer_cli() {
        Ok(()) => {
            println!("✅ VS Code is installed and available");
            println!("✅ All requirements met!");
        }
        Err(e) => {
            println!("❌ {}", e);
            return Err("System requirements not met".into());
        }
    }

    Ok(())
}

fn handle_tui_mode(config_manager: &ConfigManager) -> Result<(), Box<dyn std::error::Error>> {
    let config = config_manager.load_or_create_config()?;

    let mut app = TuiApp::new();

    match app.run(&config)? {
        Some(index) => {
            if let Some(path) = config.recent_paths.get(index) {
                println!("Selected path: {}", path.display());
                exec_devcontainer(path)?;
            } else {
                println!("Invalid selection.");
            }
        }
        None => {
            println!("Goodbye!");
        }
    }

    Ok(())
}
