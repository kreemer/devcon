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
use tracing::{Level, trace};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::command::*;

mod command;
mod config;
mod devcontainer;
mod driver;
mod feature;
mod workspace;

#[derive(Parser, Debug)]
#[command(
    name = "devcon",
    author = "kreemer",
    about = "A CLI tool for managing development containers",
    long_about = None,
    version = env!("CARGO_PKG_VERSION")
)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
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

        /// Path to the build directory.
        #[arg(short, long, help = "Path to the build directory.")]
        build_path: Option<PathBuf>,
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
    /// Builds and starts a development container for the specified path
    #[command(about = "Build and start a development container (combines build + start)")]
    Up {
        /// Path to the project directory containing .devcontainer configuration
        #[arg(
            help = "Path to the project directory. If not provided, uses current directory.",
            value_name = "PATH"
        )]
        path: Option<PathBuf>,

        /// Path to the build directory.
        #[arg(short, long, help = "Path to the build directory.")]
        build_path: Option<PathBuf>,
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
    /// Starts the control server for agent connections
    #[command(about = "Start the control server for managing agent connections")]
    Serve {
        /// Port to listen on
        #[arg(
            help = "Port to listen on for agent connections",
            long,
            short,
            default_value = "15000"
        )]
        port: u16,
    },
}

fn main() -> anyhow::Result<()> {
    let indicatif_layer = IndicatifLayer::new();
    let cli = Cli::parse();
    let level = match cli.debug {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    // Configure logging: third-party crates only log at trace level, our crate uses the configured level
    let third_party_level = if level == Level::TRACE {
        "trace"
    } else {
        "error"
    };
    let filter = EnvFilter::new(format!(
        "{}={},reqwest={},hyper={},h2={},tower={}",
        env!("CARGO_PKG_NAME").replace('-', "_"),
        level,
        third_party_level,
        third_party_level,
        third_party_level,
        third_party_level
    ));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .with(filter)
        .init();

    trace!("Starting devcon with CLI args: {:?}", cli);

    match &cli.command {
        Commands::Build { path, build_path } => {
            handle_build_command(
                path.clone().unwrap_or(PathBuf::from(".").to_path_buf()),
                build_path.clone(),
            )?;
        }
        Commands::Start { path } => {
            handle_start_command(path.clone().unwrap_or(PathBuf::from(".").to_path_buf()))?;
        }
        Commands::Up { path, build_path } => {
            handle_up_command(
                path.clone().unwrap_or(PathBuf::from(".").to_path_buf()),
                build_path.clone(),
            )?;
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
        Commands::Serve { port } => {
            handle_serve_command(*port)?;
        }
    }

    Ok(())
}
