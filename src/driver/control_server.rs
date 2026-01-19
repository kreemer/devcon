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
use devcon_proto::agent_message::Message as ProtoMessage;
use devcon_proto::AgentMessage;
use prost::Message;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, Shutdown};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info, warn};

/// Manages active port forwarding sessions
#[derive(Clone)]
struct PortForwardManager {
    /// Map of local_port -> (container_stream, container_port)
    forwards: Arc<Mutex<HashMap<u16, (Arc<Mutex<TcpStream>>, u16)>>>,
}

impl PortForwardManager {
    fn new() -> Self {
        Self {
            forwards: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start forwarding a port through the control connection
    fn start_forward(&self, local_port: u16, container_port: u16, stream: Arc<Mutex<TcpStream>>) -> Result<()> {
        let mut forwards = self.forwards.lock().unwrap();
        
        if forwards.contains_key(&local_port) {
            bail!("Port {} is already being forwarded", local_port);
        }

        // Start the local listener for this port
        let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port))
            .context(format!("Failed to bind to port {}", local_port))?;
        
        info!("Listening on 127.0.0.1:{} for connections to forward to container port {}", local_port, container_port);
        
        // Store the forward mapping
        forwards.insert(local_port, (stream.clone(), container_port));
        
        // Spawn thread to accept connections on this port
        let stream_clone = stream.clone();
        let forwards_clone = self.forwards.clone();
        thread::spawn(move || {
            for incoming_stream in listener.incoming() {
                match incoming_stream {
                    Ok(mut client_stream) => {
                        let agent_stream = stream_clone.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_forwarded_connection(&mut client_stream, agent_stream, container_port) {
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
fn handle_forwarded_connection(
    client_stream: &mut TcpStream,
    agent_stream: Arc<Mutex<TcpStream>>,
    container_port: u16,
) -> Result<()> {
    debug!("Handling forwarded connection to container port {}", container_port);
    
    // Send port forward request to agent
    let message = AgentMessage {
        message: Some(ProtoMessage::StartPortForward(devcon_proto::StartPortForward {
            port: container_port as u32,
        })),
    };
    
    let mut agent = agent_stream.lock().unwrap();
    send_message(&mut *agent, &message)?;
    
    // Now we need to tunnel data bidirectionally
    // For now, just log that we would tunnel
    debug!("Would tunnel data between client and container port {}", container_port);
    
    Ok(())
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

/// Read a protobuf message from a TCP stream with length prefix
fn read_message(stream: &mut TcpStream) -> Result<AgentMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    
    let message = AgentMessage::decode(&buf[..])?;
    Ok(message)
}

/// Handle a single agent connection
fn handle_agent_connection(mut stream: TcpStream, manager: PortForwardManager) -> Result<()> {
    let peer_addr = stream.peer_addr()?;
    info!("New agent connection from {}", peer_addr);
    
    let stream_arc = Arc::new(Mutex::new(stream.try_clone()?));
    
    loop {
        match read_message(&mut stream) {
            Ok(message) => {
                match message.message {
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
                        // Could implement URL opening here
                    }
                    None => {
                        warn!("Received message with no content");
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("UnexpectedEof") || e.to_string().contains("connection reset") {
                    info!("Agent connection closed from {}", peer_addr);
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
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .context(format!("Failed to bind to port {}", port))?;
    
    info!("Control server listening on 127.0.0.1:{}", port);
    
    let manager = PortForwardManager::new();
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let manager_clone = manager.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_agent_connection(stream, manager_clone) {
                        error!("Error handling agent connection: {}", e);
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
