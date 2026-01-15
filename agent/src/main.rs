//! DevCon Port Forwarding Agent
//!
//! This agent runs inside the container and sends port forwarding messages to the host.

use clap::{Parser, Subcommand};
use devcon_proto::{AgentMessage, OpenUrl, StartPortForward, StopPortForward, agent_message};
use prost::Message;
use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "devcon-agent")]
#[command(about = "DevCon agent", long_about = None)]
struct Cli {
    /// Path to the Unix domain socket
    #[arg(short, long, env = "DEVCON_SOCKET")]
    socket: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Request the host to start forwarding a port
    StartPortForward {
        /// Port number to forward
        #[arg(value_name = "PORT")]
        port: u16,
    },
    /// Request the host to stop forwarding a port
    StopPortForward {
        /// Port number to stop forwarding
        #[arg(value_name = "PORT")]
        port: u16,
    },
    /// Request the host to open a URL in the browser
    OpenUrl {
        /// URL to open
        #[arg(value_name = "URL")]
        url: String,
    },
}

fn send_message(socket_path: &PathBuf, msg: &AgentMessage) -> io::Result<()> {
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Connect to the Unix domain socket
    let mut stream = UnixStream::connect(socket_path)?;

    // Write length-prefixed message to the socket
    let len = buf.len() as u32;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&buf)?;

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::StartPortForward { port } => {
            let msg = AgentMessage {
                message: Some(agent_message::Message::StartPortForward(StartPortForward {
                    port: port as u32,
                })),
            };
            send_message(&cli.socket, &msg)
        }
        Commands::StopPortForward { port } => {
            let msg = AgentMessage {
                message: Some(agent_message::Message::StopPortForward(StopPortForward {
                    port: port as u32,
                })),
            };
            send_message(&cli.socket, &msg)
        }
        Commands::OpenUrl { url } => {
            let msg = AgentMessage {
                message: Some(agent_message::Message::OpenUrl(OpenUrl { url })),
            };
            send_message(&cli.socket, &msg)
        }
    };

    if let Err(e) = result {
        eprintln!("Error sending message: {}", e);
        std::process::exit(1);
    }
}
