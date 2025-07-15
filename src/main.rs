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

mod config;
mod devcontainer;
mod tui;

use clap::{Parser, Subcommand};
use indicatif::ProgressBar;
use std::path::PathBuf;
use std::time::Duration;

use config::ConfigManager;
use devcontainer::{check_devcontainer_cli, shell_devcontainer, up_devcontainer};
use tui::TuiApp;

#[derive(Parser)]
#[command(
    name = "devcon",
    about = "A TUI application for managing and launching development containers",
    long_about = "DevCon helps you manage your development containers by keeping track of recent projects and providing a convenient interface to them with devcontainer-cli.",
    version = "0.1.2"
)]
struct Cli {
    /// Set config file path
    #[arg(
        long,
        short,
        help = "Path to the configuration file. Defaults to XDG config directory."
    )]
    config_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a development container for the specified path
    #[command(about = "Open a development container with the devcontainer CLI")]
    Open {
        /// Path to the project directory containing .devcontainer configuration
        #[arg(
            help = "Path to the project directory. If not provided, uses current directory.",
            value_name = "PATH"
        )]
        path: Option<PathBuf>,
    },
    /// Execs a shell in a development container for the specified path
    #[command(about = "Exec a shell in a development container with the devcontainer CLI")]
    Shell {
        /// Path to the project directory containing .devcontainer configuration
        #[arg(
            help = "Path to the project directory. If not provided, uses current directory.",
            value_name = "PATH"
        )]
        path: Option<PathBuf>,

        /// Environment variables which will be processed. Each should be denoted by KEY=VALUE
        #[arg(
            help = "Environment variables which will be processed. Each should be denoted by KEY=VALUE.",
            value_name = "PATH"
        )]
        env: Vec<String>,
    },
    /// Check if the required tools are available
    #[command(about = "Check if DevContainer CLI is properly installed and available")]
    Check,
    /// Manage configuration settings
    #[command(subcommand, about = "Manage configuration settings for DevCon")]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Manage dotfiles repository configuration
    #[command(subcommand, about = "Configure dotfiles repository for devcontainers")]
    Dotfiles(DotfilesCommands),
    /// Manage additional features configuration
    #[command(subcommand, about = "Configure additional features for devcontainers")]
    Features(FeaturesCommands),
    /// Manage additional features configuration
    #[command(
        subcommand,
        about = "Configure additional env variables for devcontainers"
    )]
    Envs(EnvsCommands),
}

#[derive(Subcommand)]
enum DotfilesCommands {
    /// Set dotfiles repository URL
    #[command(about = "Set the dotfiles repository URL")]
    Set {
        /// Repository URL (e.g., https://github.com/user/dotfiles)
        #[arg(help = "The dotfiles repository URL")]
        repo_url: String,
    },
    /// Remove dotfiles repository configuration
    #[command(about = "Remove the dotfiles repository configuration")]
    Clear,
    /// Show current dotfiles repository configuration
    #[command(about = "Show current dotfiles repository configuration")]
    Show,
}

#[derive(Subcommand)]
enum FeaturesCommands {
    /// Add an additional feature
    #[command(about = "Add an additional feature")]
    Add {
        /// Feature name (e.g., ghcr.io/devcontainers/features/github-cli:1)
        #[arg(help = "The feature identifier")]
        feature: String,
        /// Feature version or configuration
        #[arg(help = "The feature version or configuration value")]
        value: String,
    },
    /// Remove an additional feature
    #[command(about = "Remove an additional feature")]
    Remove {
        /// Feature name to remove
        #[arg(help = "The feature identifier to remove")]
        feature: String,
    },
    /// List all configured additional features
    #[command(about = "List all configured additional features")]
    List,
    /// Clear all additional features
    #[command(about = "Clear all additional features")]
    Clear,
}

#[derive(Subcommand)]
enum EnvsCommands {
    /// Add an additional feature
    #[command(about = "Add an additional env variable")]
    Add {
        /// Env name (e.g., PATH)
        #[arg(help = "The env identifier")]
        name: String,
        /// Env variable value
        #[arg(help = "The env variable value")]
        value: String,
        /// Context for the env variable
        #[arg(help = "The context for the env variable (all, up or exec)")]
        context: Option<String>,
    },
    /// Remove an additional feature
    #[command(about = "Remove an env variable")]
    Remove {
        /// Feature name to remove
        #[arg(help = "The env index to remove")]
        index: usize,
    },
    /// List all configured env variables
    #[command(about = "List all configured env variables")]
    List,
    /// Clear all env variables
    #[command(about = "Clear all env variables")]
    Clear,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let config_file = if let Some(path) = cli.config_file {
        path
    } else {
        // Use XDG config directory if no path is provided
        let xdg_dirs = xdg::BaseDirectories::with_prefix("devcon")
            .get_config_home()
            .expect("Failed to create XDG base directories");
        xdg_dirs.join("config.yaml")
    };

    let config_manager = ConfigManager::new(config_file).unwrap();

    match &cli.command {
        Some(Commands::Open { path }) => {
            let bar = ProgressBar::new_spinner();
            bar.enable_steady_tick(Duration::from_millis(100));
            handle_open_command(&config_manager, path.as_ref())?;
            bar.finish();
        }
        Some(Commands::Shell { path, env }) => {
            handle_shell_command(&config_manager, path.as_ref(), env)?;
        }
        Some(Commands::Check) => {
            handle_check_command()?;
        }
        Some(Commands::Config(config_cmd)) => {
            handle_config_command(&config_manager, config_cmd)?;
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
    up_devcontainer(&updated_config.recent_paths[0], &updated_config)?;

    Ok(())
}

fn handle_shell_command(
    config_manager: &ConfigManager,
    path: Option<&PathBuf>,
    env: &Vec<String>,
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

    // Execute devcontainer
    shell_devcontainer(&open_path, env, &config)?;

    Ok(())
}

fn handle_check_command() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking system requirements...");

    match check_devcontainer_cli() {
        Ok(()) => {
            println!("✅ All requirements met!");
        }
        Err(e) => {
            println!("❌ {e}");
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
                up_devcontainer(path, &config)?;
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

fn handle_config_command(
    config_manager: &ConfigManager,
    config_cmd: &ConfigCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match config_cmd {
        ConfigCommands::Dotfiles(dotfiles_cmd) => {
            handle_dotfiles_command(config_manager, dotfiles_cmd)?;
        }
        ConfigCommands::Features(features_cmd) => {
            handle_features_command(config_manager, features_cmd)?;
        }
        ConfigCommands::Envs(envs_cmd) => {
            handle_envs_command(config_manager, envs_cmd)?;
        }
    }
    Ok(())
}

fn handle_dotfiles_command(
    config_manager: &ConfigManager,
    dotfiles_cmd: &DotfilesCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match dotfiles_cmd {
        DotfilesCommands::Set { repo_url } => {
            let config = config_manager.load_or_create_config()?;
            config_manager.set_dotfiles_repo(config, Some(repo_url.clone()))?;
            println!("✅ Dotfiles repository set to: {repo_url}");
        }
        DotfilesCommands::Clear => {
            let config = config_manager.load_or_create_config()?;
            config_manager.set_dotfiles_repo(config, None)?;
            println!("✅ Dotfiles repository configuration cleared");
        }
        DotfilesCommands::Show => {
            let config = config_manager.load_or_create_config()?;
            match &config.dotfiles_repo {
                Some(repo) => println!("Current dotfiles repository: {repo}"),
                None => println!("No dotfiles repository configured"),
            }
        }
    }
    Ok(())
}

fn handle_features_command(
    config_manager: &ConfigManager,
    features_cmd: &FeaturesCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match features_cmd {
        FeaturesCommands::Add { feature, value } => {
            let config = config_manager.load_or_create_config()?;
            config_manager.add_feature(config, feature.clone(), value.clone())?;
            println!("✅ Added feature: {feature} = {value}");
        }
        FeaturesCommands::Remove { feature } => {
            let config = config_manager.load_or_create_config()?;
            config_manager.remove_feature(config, feature.clone())?;
            println!("✅ Removed feature: {feature}");
        }
        FeaturesCommands::List => {
            let config = config_manager.load_or_create_config()?;
            if config.additional_features.is_empty() {
                println!("No additional features configured");
            } else {
                println!("Configured additional features:");
                for (feature, value) in &config.additional_features {
                    println!("  {feature} = {value}");
                }
            }
        }
        FeaturesCommands::Clear => {
            let config = config_manager.load_or_create_config()?;
            config_manager.clear_features(config)?;
            println!("✅ All additional features cleared");
        }
    }
    Ok(())
}

fn handle_envs_command(
    config_manager: &ConfigManager,
    envs_cmd: &EnvsCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match envs_cmd {
        EnvsCommands::Add {
            name,
            value,
            context,
        } => {
            let config = config_manager.load_or_create_config()?;
            let context = context
                .as_ref()
                .and_then(|c| c.parse::<config::DevContainerContext>().ok())
                .unwrap_or(config::DevContainerContext::default());
            config_manager.add_env(config, name.clone(), value.clone(), Some(context.clone()))?;
            println!("✅ Added env: {name} = {value} (in context: {context:?})");
        }
        EnvsCommands::Remove { index } => {
            let config = config_manager.load_or_create_config()?;
            config_manager.remove_env(config, *index)?;
            println!("✅ Removed env: {index}");
        }
        EnvsCommands::List => {
            let config = config_manager.load_or_create_config()?;
            if config.env.is_empty() {
                println!("No additional env vars found");
            } else {
                println!("Configured additional env vars:");
                for env in &config.env {
                    println!(
                        "  {} = {} (in context: {:?})",
                        env.name, env.value, env.context
                    );
                }
            }
        }
        EnvsCommands::Clear => {
            let config = config_manager.load_or_create_config()?;
            config_manager.clear_env(config)?;
            println!("✅ All envs cleared");
        }
    }
    Ok(())
}
