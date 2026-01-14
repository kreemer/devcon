//! DevCon Port Forwarding Agent
//!
//! This agent runs inside the container and continuously monitors for open ports.
//! It communicates with the host via stdout using protobuf messages.

use devcon_proto::{AgentMessage, StartPortForward, StopPortForward, agent_message};
use prost::Message;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn get_listening_ports() -> io::Result<Vec<u16>> {
    let mut ports = Vec::new();

    // Try reading /proc/net/tcp and /proc/net/tcp6
    for file in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = fs::read_to_string(file) {
            for line in content.lines().skip(1) {
                // Skip header
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    continue;
                }

                // Column 1 is local_address in format "0100007F:1F90" (127.0.0.1:8080)
                // Column 3 is st (state), 0A = LISTEN
                if parts.get(3) == Some(&"0A") {
                    if let Some(local_addr) = parts.get(1) {
                        if let Some(port_hex) = local_addr.split(':').nth(1) {
                            if let Ok(port) = u16::from_str_radix(port_hex, 16) {
                                if port > 0 && !ports.contains(&port) {
                                    ports.push(port);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ports)
}

fn send_message(msg: &AgentMessage) {
    let mut buf = Vec::new();
    if msg.encode(&mut buf).is_ok() {
        // Write length-prefixed message
        let len = buf.len() as u32;
        if io::stdout().write_all(&len.to_be_bytes()).is_ok() {
            let _ = io::stdout().write_all(&buf);
            let _ = io::stdout().flush();
        }
    }
}

fn main() {
    eprintln!("[devcon-agent] Starting port monitoring agent");

    let known_ports: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));
    let known_ports_clone = Arc::clone(&known_ports);
    let check_interval = Duration::from_secs(2);

    // Start monitoring thread
    thread::spawn(move || {
        loop {
            match get_listening_ports() {
                Ok(current_ports) => {
                    let current_set: HashSet<u16> = current_ports.iter().copied().collect();
                    let mut known = known_ports_clone.lock().unwrap();

                    // Find new ports
                    for port in current_set.difference(&known) {
                        eprintln!("[devcon-agent] Discovered new port: {}", port);
                        let msg = AgentMessage {
                            message: Some(agent_message::Message::StartPortForward(
                                StartPortForward { port: *port as u32 },
                            )),
                        };
                        send_message(&msg);
                    }

                    // Find closed ports
                    for port in known.difference(&current_set) {
                        eprintln!("[devcon-agent] Port closed: {}", port);
                        let msg = AgentMessage {
                            message: Some(agent_message::Message::StopPortForward(
                                StopPortForward { port: *port as u32 },
                            )),
                        };
                        send_message(&msg);
                    }

                    *known = current_set;
                }
                Err(e) => {
                    eprintln!("[devcon-agent] Error reading ports: {}", e);
                }
            }

            thread::sleep(check_interval);
        }
    });

    // Listen for commands from host on stdin
    let stdin = io::stdin();
    let mut stdin_handle = stdin.lock();
    let mut len_buf = [0u8; 4];

    loop {
        // Read message length
        if stdin_handle.read_exact(&mut len_buf).is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;

        // Read message data
        let mut msg_buf = vec![0u8; len];
        if stdin_handle.read_exact(&mut msg_buf).is_err() {
            break;
        }

        // Decode protobuf message
        if let Ok(msg) = AgentMessage::decode(&msg_buf[..]) {
            match msg.message {
                Some(agent_message::Message::OpenUrl(open_url)) => {
                    eprintln!(
                        "[devcon-agent] Received request to open URL: {}",
                        open_url.url
                    );
                    // In a real implementation, this might trigger browser opening
                }
                _ => {
                    eprintln!("[devcon-agent] Received unexpected message type");
                }
            }
        }
    }

    eprintln!("[devcon-agent] Agent stopped");
}
