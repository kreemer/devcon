use std::{
    net::{TcpListener, TcpStream},
    process::Command,
};

use devcon_proto::{
    TcpWithSize,
    protos::{
        Request, request::RequestTypeOneof::Browser, request::RequestTypeOneof::StartSocket,
        request::RequestTypeOneof::StopSocket,
    },
};
use protobuf::Parse;

pub fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let recieve_buffer = stream.recieve()?;
    let request = Request::parse(&recieve_buffer).unwrap();
    match request.request_type() {
        Browser(browser) => {
            let url = browser.url().to_str().unwrap();
            // Try to open the URL using the system's default browser
            let result = if cfg!(target_os = "macos") {
                Command::new("open").arg(url).output()
            } else if cfg!(target_os = "windows") {
                Command::new("cmd").args(["/c", "start", url]).output()
            } else {
                // Linux and other Unix-like systems
                // Try xdg-open first, then fallback to other options
                Command::new("xdg-open").arg(url).output()
            };

            match result {
                Ok(output) => {
                    if output.status.success() {
                        println!("✅ Successfully opened URL: {url}");
                    } else {
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        eprintln!("❌ Failed to open URL: {error_msg}");
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to execute browser command: {e}");
                }
            }
        }
        StartSocket(socket) => {
            let port = socket.port_number();
            let peer = socket.peer_address();
            println!("Port Number: {}", port);
            println!("Peer address: {}", peer);

            let effective_port: u16;

            match u16::try_from(port) {
                Ok(port) => {
                    if openport::is_free_tcp(port) {
                        effective_port = port
                    } else {
                        effective_port = 0;
                    }
                }
                Err(_) => {
                    effective_port = 0;
                }
            }

            let listener = TcpListener::bind(format!("0.0.0.0:{}", effective_port)).expect("fail");
            loop {
                TcpStream::connect(peer);
            }
        }
        StopSocket(socket) => {
            let port = socket.port_number();
            println!("Port Number: {}", port);
        }
        _ => panic!("Non request type found"),
    }

    Ok(())
}
