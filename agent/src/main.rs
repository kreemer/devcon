//! DevCon Port Forwarding Agent
//!
//! This agent runs inside the container and continuously monitors for open ports.
//! It communicates with the host via stdout, reporting ports that should be forwarded.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Message {
    /// Agent -> Host: Report newly discovered port
    PortDiscovered { port: u16 },
    /// Agent -> Host: Report port that closed
    PortClosed { port: u16 },
    /// Host -> Agent: Acknowledge port forwarding started
    PortForwarded { port: u16, host_port: u16 },
    /// Host -> Agent: Request current port list
    ListPorts,
    /// Agent -> Host: Response to ListPorts
    PortList { ports: Vec<u16> },
    /// Host -> Agent: Shutdown agent
    Shutdown,
}

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

fn send_message(msg: &Message) {
    if let Ok(json) = serde_json::to_string(msg) {
        println!("{}", json);
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
                        send_message(&Message::PortDiscovered { port: *port });
                    }
                    
                    // Find closed ports
                    for port in known.difference(&current_set) {
                        eprintln!("[devcon-agent] Port closed: {}", port);
                        send_message(&Message::PortClosed { port: *port });
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
    for line in stdin.lock().lines() {
        if let Ok(line) = line {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                match msg {
                    Message::ListPorts => {
                        let known = known_ports.lock().unwrap();
                        let ports: Vec<u16> = known.iter().copied().collect();
                        send_message(&Message::PortList { ports });
                    }
                    Message::PortForwarded { port, host_port } => {
                        eprintln!("[devcon-agent] Port {} forwarded to host:{}", port, host_port);
                    }
                    Message::Shutdown => {
                        eprintln!("[devcon-agent] Shutdown requested");
                        break;
                    }
                    _ => {
                        eprintln!("[devcon-agent] Unexpected message type");
                    }
                }
            }
        }
    }
    
    eprintln!("[devcon-agent] Agent stopped");
}
