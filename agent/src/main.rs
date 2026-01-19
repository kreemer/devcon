//! DevCon Port Forwarding Agent
//!
//! This agent runs inside the container and communicates with the host control server via TCP.

use clap::{Parser, Subcommand};
use devcon_proto::{AgentMessage, OpenUrl, StartPortForward, StopPortForward, agent_message};
use prost::Message;
use std::io::{self, Read, Write};
use std::net::TcpStream;

#[derive(Parser)]
#[command(name = "devcon-agent")]
#[command(about = "DevCon agent", long_about = None)]
struct Cli {
    /// Host address for the control server
    #[arg(
        short = 'H',
        long,
        env = "DEVCON_CONTROL_HOST",
        default_value = "host.docker.internal"
    )]
    control_host: String,

    /// Port for the control server
    #[arg(
        short = 'p',
        long,
        env = "DEVCON_CONTROL_PORT",
        default_value = "15000"
    )]
    control_port: u16,

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
    /// Run as a daemon, maintaining connection to control server
    Daemon,
}

/// Send a protobuf message over a TCP stream with length prefix
fn send_message(stream: &mut TcpStream, msg: &AgentMessage) -> io::Result<()> {
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let len = buf.len() as u32;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&buf)?;
    stream.flush()?;

    Ok(())
}

/// Read a protobuf message from a TCP stream with length prefix
fn read_message(stream: &mut TcpStream) -> io::Result<AgentMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;

    AgentMessage::decode(&buf[..]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Connect to the control server
fn connect_to_control_server(host: &str, port: u16) -> io::Result<TcpStream> {
    let addr = format!("{}:{}", host, port);
    eprintln!("Connecting to control server at {}", addr);
    TcpStream::connect(addr)
}

/// Handle port forwarding requests - connect to local service and proxy data
fn handle_tunnel_request(stream: &mut TcpStream, port: u16) -> io::Result<()> {
    eprintln!(
        "Tunnel request received for port {}, connecting to localhost:{} in container",
        port, port
    );

    // Connect to the local service in the container
    let local_addr = format!("127.0.0.1:{}", port);
    let local_stream = match TcpStream::connect(&local_addr) {
        Ok(s) => {
            eprintln!("Connected to local service at {}", local_addr);
            s
        }
        Err(e) => {
            eprintln!(
                "Failed to connect to local service at {}: {}",
                local_addr, e
            );
            return Err(e);
        }
    };

    // Clone streams for bidirectional proxying
    let mut stream_read = stream.try_clone()?;
    let mut stream_write = stream.try_clone()?;
    let mut local_read = local_stream.try_clone()?;
    let mut local_write = local_stream.try_clone()?;

    // Spawn thread to copy from control connection to local service
    let handle = std::thread::spawn(move || {
        let result = std::io::copy(&mut stream_read, &mut local_write);
        let _ = local_write.shutdown(std::net::Shutdown::Write);
        result
    });

    // Copy from local service to control connection in this thread
    let result = std::io::copy(&mut local_read, &mut stream_write);
    let _ = stream_write.shutdown(std::net::Shutdown::Write);

    // Wait for the other direction to complete
    let _ = handle.join();

    eprintln!("Tunnel closed for port {}", port);
    result.map(|_| ())
}

/// Run port forward daemon for a specific port
fn run_port_forward_daemon(stream: &mut TcpStream, port: u16) -> io::Result<()> {
    eprintln!("Port forward daemon running for port {}", port);

    // Keep the connection alive and handle tunnel requests
    loop {
        match read_message(stream) {
            Ok(message) => {
                match message.message {
                    Some(agent_message::Message::StartPortForward(fwd)) => {
                        eprintln!("Received tunnel request for port {}", fwd.port);
                        if let Err(e) = handle_tunnel_request(stream, fwd.port as u16) {
                            eprintln!("Error handling tunnel: {}", e);
                            // Connection likely broken, exit
                            break;
                        }
                    }
                    _ => {
                        eprintln!("Received unexpected message: {:?}", message);
                    }
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("Control server connection closed");
                    break;
                }
                eprintln!("Error reading from control server: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Run the agent as a daemon, maintaining connection to control server
fn run_daemon(host: &str, port: u16) -> io::Result<()> {
    let mut stream = connect_to_control_server(host, port)?;
    eprintln!("Connected to control server");

    // Keep the connection alive and handle any incoming messages
    loop {
        match read_message(&mut stream) {
            Ok(message) => {
                eprintln!("Received message from host: {:?}", message);
                // Handle incoming messages from host (e.g., port forward requests)
                match message.message {
                    Some(agent_message::Message::StartPortForward(fwd)) => {
                        eprintln!("Received tunnel request for port {}", fwd.port);
                        if let Err(e) = handle_tunnel_request(&mut stream, fwd.port as u16) {
                            eprintln!("Error handling tunnel: {}", e);
                        }
                    }
                    _ => {
                        eprintln!("Received message: {:?}", message);
                    }
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    eprintln!("Control server connection closed");
                    break;
                }
                eprintln!("Error reading from control server: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::StartPortForward { port } => {
            match connect_to_control_server(&cli.control_host, cli.control_port) {
                Ok(mut stream) => {
                    eprintln!("Requesting port forward for port {}", port);
                    let msg = AgentMessage {
                        message: Some(agent_message::Message::StartPortForward(StartPortForward {
                            port: port as u32,
                        })),
                    };
                    match send_message(&mut stream, &msg) {
                        Ok(_) => {
                            eprintln!("Port forward request sent, keeping connection alive...");
                            // Keep connection alive and handle any reverse tunnel requests
                            run_port_forward_daemon(&mut stream, port)
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            }
        }
        Commands::StopPortForward { port } => {
            match connect_to_control_server(&cli.control_host, cli.control_port) {
                Ok(mut stream) => {
                    let msg = AgentMessage {
                        message: Some(agent_message::Message::StopPortForward(StopPortForward {
                            port: port as u32,
                        })),
                    };
                    send_message(&mut stream, &msg)
                }
                Err(e) => Err(e),
            }
        }
        Commands::OpenUrl { url } => {
            match connect_to_control_server(&cli.control_host, cli.control_port) {
                Ok(mut stream) => {
                    let msg = AgentMessage {
                        message: Some(agent_message::Message::OpenUrl(OpenUrl { url })),
                    };
                    send_message(&mut stream, &msg)
                }
                Err(e) => Err(e),
            }
        }
        Commands::Daemon => run_daemon(&cli.control_host, cli.control_port),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
