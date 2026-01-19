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

//! # Control Server
//!
//! This module implements the TCP control server that accepts connections from
//! container agents and manages port forwarding requests.

use anyhow::{Context, Result, bail};
use devcon_proto::AgentMessage;
use devcon_proto::agent_message::Message as ProtoMessage;
use prost::Message;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info, warn};

/// Manages active port forwarding sessions
#[derive(Clone)]
struct PortForwardManager {
    /// Map of local_port -> (agent_stream, container_port, tunnel_id_counter)
    forwards: Arc<Mutex<HashMap<u16, (Arc<Mutex<TcpStream>>, u16, Arc<AtomicU32>)>>>,
    /// Map of tunnel_id -> pending client stream
    pending_tunnels: Arc<Mutex<HashMap<u32, TcpStream>>>,
    /// Control server port for tunnel connections
    control_port: u16,
}

impl PortForwardManager {
    fn new(control_port: u16) -> Self {
        Self {
            forwards: Arc::new(Mutex::new(HashMap::new())),
            pending_tunnels: Arc::new(Mutex::new(HashMap::new())),
            control_port,
        }
    }

    /// Start forwarding a port through the control connection
    fn start_forward(
        &self,
        local_port: u16,
        container_port: u16,
        stream: Arc<Mutex<TcpStream>>,
    ) -> Result<()> {
        let mut forwards = self.forwards.lock().unwrap();

        if forwards.contains_key(&local_port) {
            bail!("Port {} is already being forwarded", local_port);
        }

        // Start the local listener for this port
        let listener = TcpListener::bind(format!("0.0.0.0:{}", local_port))
            .context(format!("Failed to bind to port {}", local_port))?;

        info!(
            "Listening on 0.0.0.0:{} for connections to forward to container port {}",
            local_port, container_port
        );

        // Store the forward mapping
        let tunnel_id_counter = Arc::new(AtomicU32::new(1));
        forwards.insert(
            local_port,
            (stream.clone(), container_port, tunnel_id_counter.clone()),
        );

        // Spawn thread to accept connections on this port
        let stream_clone = stream.clone();
        let forwards_clone = self.forwards.clone();
        let pending_tunnels = self.pending_tunnels.clone();
        let control_port = self.control_port;

        thread::spawn(move || {
            for incoming_stream in listener.incoming() {
                match incoming_stream {
                    Ok(client_stream) => {
                        let agent_stream = stream_clone.clone();
                        let tunnel_id = tunnel_id_counter.fetch_add(1, Ordering::SeqCst);
                        let pending_clone = pending_tunnels.clone();

                        thread::spawn(move || {
                            if let Err(e) = handle_forwarded_connection(
                                client_stream,
                                agent_stream,
                                container_port,
                                tunnel_id,
                                pending_clone,
                                control_port,
                            ) {
                                error!("Error handling forwarded connection: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error accepting connection: {}", e);
                        // Check if we should stop listening (forward was stopped)
                        let forwards = forwards_clone.lock().unwrap();
                        if !forwards.contains_key(&local_port) {
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Get and remove a pending tunnel client stream
    fn take_pending_tunnel(&self, tunnel_id: u32) -> Option<TcpStream> {
        let mut pending = self.pending_tunnels.lock().unwrap();
        pending.remove(&tunnel_id)
    }

    /// Stop forwarding a port
    fn stop_forward(&self, local_port: u16) -> Result<()> {
        let mut forwards = self.forwards.lock().unwrap();

        if forwards.remove(&local_port).is_some() {
            info!("Stopped forwarding port {}", local_port);
            Ok(())
        } else {
            bail!("Port {} is not being forwarded", local_port);
        }
    }
}

/// Handle a forwarded connection from host to container
/// This sends a tunnel request to the agent and waits for it to connect back
fn handle_forwarded_connection(
    client_stream: TcpStream,
    agent_stream: Arc<Mutex<TcpStream>>,
    container_port: u16,
    tunnel_id: u32,
    pending_tunnels: Arc<Mutex<HashMap<u32, TcpStream>>>,
    control_port: u16,
) -> Result<()> {
    debug!(
        "Handling forwarded connection to container port {}, tunnel_id={}",
        container_port, tunnel_id
    );

    // Store the client stream as pending
    {
        let mut pending = pending_tunnels.lock().unwrap();
        pending.insert(tunnel_id, client_stream);
        debug!(
            "Stored pending client for tunnel_id={}, total pending: {}",
            tunnel_id,
            pending.len()
        );
    }

    // Send tunnel request to agent over control connection
    let message = AgentMessage {
        message: Some(ProtoMessage::TunnelRequest(devcon_proto::TunnelRequest {
            port: container_port as u32,
            tunnel_id,
        })),
    };

    let mut agent = agent_stream.lock().unwrap();
    send_message(&mut *agent, &message)?;
    drop(agent); // Release lock immediately

    debug!(
        "Sent tunnel request to agent for port {}, tunnel_id={}, agent should connect back on port {}",
        container_port, tunnel_id, control_port
    );

    // Wait up to 5 seconds for the tunnel to be established
    // This keeps the client stream alive in pending_tunnels
    let start = std::time::Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Check if tunnel was taken (meaning agent connected)
        {
            let pending = pending_tunnels.lock().unwrap();
            if !pending.contains_key(&tunnel_id) {
                debug!("Tunnel {} successfully established", tunnel_id);
                return Ok(());
            }
        }

        // Timeout after 5 seconds
        if start.elapsed().as_secs() > 5 {
            warn!("Timeout waiting for tunnel {} to be established", tunnel_id);
            // Remove from pending to clean up
            let mut pending = pending_tunnels.lock().unwrap();
            pending.remove(&tunnel_id);
            bail!("Tunnel establishment timeout");
        }
    }
}

/// Send a protobuf message over a TCP stream with length prefix
fn send_message(stream: &mut TcpStream, message: &AgentMessage) -> Result<()> {
    let mut buf = Vec::new();
    message.encode(&mut buf)?;

    let len = buf.len() as u32;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&buf)?;
    stream.flush()?;

    Ok(())
}

/// Open a URL in the default browser
fn open_url(url: &str) -> Result<()> {
    info!("Opening URL in browser: {}", url);
    open::that(url).context("Failed to open URL in browser")?;
    info!("Successfully opened URL");
    Ok(())
}

/// Read a protobuf message from a TCP stream with length prefix
fn read_message(stream: &mut TcpStream) -> Result<AgentMessage> {
    let mut len_buf = [0u8; 4];

    // Try to read the length prefix
    match stream.read_exact(&mut len_buf) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            bail!("Connection closed while reading message length");
        }
        Err(e) => return Err(e.into()),
    }

    let len = u32::from_be_bytes(len_buf) as usize;

    // Validate message length to prevent excessive memory allocation
    if len == 0 {
        bail!("Received zero-length message");
    }
    if len > 10 * 1024 * 1024 {
        bail!("Message too large: {} bytes (max 10MB)", len);
    }

    let mut buf = vec![0u8; len];

    // Try to read the message body
    match stream.read_exact(&mut buf) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            bail!(
                "Connection closed while reading message body (expected {} bytes)",
                len
            );
        }
        Err(e) => return Err(e.into()),
    }

    let message = AgentMessage::decode(&buf[..]).context("Failed to decode protobuf message")?;
    Ok(message)
}

/// Handle incoming connection - determine if it's a control connection or tunnel connection
fn handle_connection(mut stream: TcpStream, manager: PortForwardManager) -> Result<()> {
    let peer_addr = stream.peer_addr()?;

    // Peek at first 4 bytes to determine connection type
    let mut peek_buf = [0u8; 4];
    match stream.peek(&mut peek_buf) {
        Ok(n) if n == 4 => {
            let first_u32 = u32::from_be_bytes(peek_buf);
            const TUNNEL_MAGIC: u32 = 0x54554E4E; // ASCII "TUNN"

            debug!(
                "Connection from {}, first 4 bytes: 0x{:08x}",
                peer_addr, first_u32
            );

            // Check for tunnel magic prefix
            if first_u32 == TUNNEL_MAGIC {
                // This is a tunnel connection
                stream.read_exact(&mut peek_buf)?; // Consume the magic bytes

                // Read the tunnel_id
                let mut tunnel_id_buf = [0u8; 4];
                stream.read_exact(&mut tunnel_id_buf)?;
                let tunnel_id = u32::from_be_bytes(tunnel_id_buf);

                debug!(
                    "Detected tunnel connection from {} with tunnel_id={}",
                    peer_addr, tunnel_id
                );
                handle_tunnel_connection(stream, tunnel_id, manager)
            } else {
                // This is a control connection (protobuf message length)
                debug!("Detected control connection from {}", peer_addr);
                handle_agent_connection(stream, manager)
            }
        }
        _ => {
            // Can't peek, assume control connection
            debug!(
                "Cannot peek connection from {}, assuming control connection",
                peer_addr
            );
            handle_agent_connection(stream, manager)
        }
    }
}

/// Handle a tunnel connection from agent
fn handle_tunnel_connection(
    agent_stream: TcpStream,
    tunnel_id: u32,
    manager: PortForwardManager,
) -> Result<()> {
    debug!("Handling tunnel connection for tunnel_id={}", tunnel_id);

    // Get the pending client stream for this tunnel_id
    let client_stream = manager.take_pending_tunnel(tunnel_id);

    if client_stream.is_none() {
        warn!("No pending client found for tunnel_id={}", tunnel_id);
        return Ok(());
    }

    let client_stream = client_stream.unwrap();
    info!(
        "Matched tunnel_id={} with pending client, starting bidirectional proxy",
        tunnel_id
    );

    // Proxy data bidirectionally
    let mut agent_read = agent_stream.try_clone()?;
    let mut agent_write = agent_stream;
    let mut client_read = client_stream.try_clone()?;
    let mut client_write = client_stream;

    // Spawn thread to copy from client to agent
    let handle = thread::spawn(move || {
        let result = std::io::copy(&mut client_read, &mut agent_write);
        let _ = agent_write.shutdown(std::net::Shutdown::Write);
        result
    });

    // Copy from agent to client in this thread
    let result = std::io::copy(&mut agent_read, &mut client_write);
    let _ = client_write.shutdown(std::net::Shutdown::Write);

    // Wait for the other direction to complete
    let _ = handle.join();

    debug!("Tunnel closed for tunnel_id={}", tunnel_id);
    result.map(|_| ()).map_err(|e| e.into())
}

/// Handle a single agent connection
fn handle_agent_connection(mut stream: TcpStream, manager: PortForwardManager) -> Result<()> {
    let peer_addr = stream.peer_addr()?;
    info!("New agent connection from {}", peer_addr);

    let stream_arc = Arc::new(Mutex::new(stream.try_clone()?));

    loop {
        match read_message(&mut stream) {
            Ok(message) => match message.message {
                Some(ProtoMessage::StartPortForward(fwd)) => {
                    let port = fwd.port as u16;
                    info!("Agent requested port forward: {}", port);

                    if let Err(e) = manager.start_forward(port, port, stream_arc.clone()) {
                        error!("Failed to start port forward: {}", e);
                    }
                }
                Some(ProtoMessage::StopPortForward(fwd)) => {
                    let port = fwd.port as u32 as u16;
                    info!("Agent requested stop port forward: {}", port);

                    if let Err(e) = manager.stop_forward(port) {
                        error!("Failed to stop port forward: {}", e);
                    }
                }
                Some(ProtoMessage::OpenUrl(url_msg)) => {
                    info!("Agent requested to open URL: {}", url_msg.url);
                    if let Err(e) = open_url(&url_msg.url) {
                        error!("Failed to open URL: {}", e);
                    }
                }
                Some(ProtoMessage::TunnelRequest(_)) => {
                    warn!(
                        "Received unexpected TunnelRequest from agent (this should only go agent->host)"
                    );
                }
                None => {
                    warn!("Received message with no content");
                }
            },
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Connection closed")
                    || err_str.contains("UnexpectedEof")
                    || err_str.contains("connection reset")
                    || err_str.contains("Connection reset")
                {
                    debug!("Agent connection closed from {}: {}", peer_addr, e);
                    info!("Agent {} disconnected", peer_addr);
                } else {
                    error!("Error reading from agent {}: {}", peer_addr, e);
                }
                break;
            }
        }
    }

    Ok(())
}

/// Start the control server on the specified port
pub fn start_control_server(port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .context(format!("Failed to bind to port {}", port))?;

    info!("Control server listening on 0.0.0.0:{}", port);

    let manager = PortForwardManager::new(port);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let manager_clone = manager.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, manager_clone) {
                        error!("Error handling connection: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
