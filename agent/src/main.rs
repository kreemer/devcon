//! DevCon Port Forwarding Agent
//!
//! This agent runs inside the container and sends port forwarding messages to the host.

use clap::{Parser, Subcommand};
use devcon_proto::{AgentMessage, OpenUrl, StartPortForward, StopPortForward, agent_message};
use prost::Message;
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "devcon-agent")]
#[command(about = "DevCon agent", long_about = None)]
struct Cli {
    /// Path to the message queue directory
    #[arg(short, long, env = "DEVCON_MESSAGE_DIR", default_value = "/var/run/devcon/messages")]
    message_dir: PathBuf,

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

fn send_message(message_dir: &PathBuf, msg: &AgentMessage) -> io::Result<()> {
    // Create message directory if it doesn't exist
    std::fs::create_dir_all(message_dir)?;
    
    // Encode the protobuf message
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Generate unique filename with timestamp and random component
    use std::time::SystemTime;
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let random = std::process::id(); // Use PID as random component
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let filename = format!("{}-{}-{}.msg", timestamp, random, nanos);
    let filepath = message_dir.join(&filename);

    // Write message to temp file, then rename atomically
    let temp_path = message_dir.join(format!(".tmp-{}-{}", random, nanos));
    std::fs::write(&temp_path, &buf)?;
    std::fs::rename(&temp_path, &filepath)?;

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
            send_message(&cli.message_dir, &msg)
        }
        Commands::StopPortForward { port } => {
            let msg = AgentMessage {
                message: Some(agent_message::Message::StopPortForward(StopPortForward {
                    port: port as u32,
                })),
            };
            send_message(&cli.message_dir, &msg)
        }
        Commands::OpenUrl { url } => {
            let msg = AgentMessage {
                message: Some(agent_message::Message::OpenUrl(OpenUrl { url })),
            };
            send_message(&cli.message_dir, &msg)
        }
    };

    if let Err(e) = result {
        eprintln!("Error sending message: {}", e);
        std::process::exit(1);
    }
}
