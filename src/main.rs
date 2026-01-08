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

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::command::*;

mod command;
mod config;
mod devcontainer;
mod driver;

#[derive(Parser)]
#[command(
    name = "devcon",
    about = "A TUI application for managing and launching development containers",
    long_about = "DevCon helps you manage your development containers by keeping track of recent projects and providing a convenient interface to them with devcontainer-cli.",
    version = "0.2.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Builds a development container for the specified path
    #[command(about = "Create a development container")]
    Build {
        /// Path to the project directory containing .devcontainer configuration
        #[arg(
            help = "Path to the project directory. If not provided, uses current directory.",
            value_name = "PATH"
        )]
        path: Option<PathBuf>,
    },

    /// Starts a development container for the specified path
    #[command(about = "Create a development container")]
    Start {
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
    /// Prints the config file location path
    #[command(about = "Show the config file location")]
    Config {
        #[arg(help = "Create the config file if it does not exist", long, short)]
        create_if_missing: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Build { path } => {
            handle_build_command(path.clone().unwrap_or(PathBuf::from(".").to_path_buf()))?;
        }
        Commands::Start { path } => {
            handle_start_command(path.clone().unwrap_or(PathBuf::from(".").to_path_buf()))?;
        }
        Commands::Shell { path, env } => {
            handle_shell_command(
                path.clone().unwrap_or(PathBuf::from(".").to_path_buf()),
                env,
            )?;
        }
        Commands::Config { create_if_missing } => {
            handle_config_command(*create_if_missing)?;
        }
    }

    Ok(())
}
