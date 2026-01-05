use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;

use clap::Parser;
use clap::Subcommand;
use devcon_proto::TcpWithSize;
use devcon_proto::protos::Browser;
use devcon_proto::protos::Request;
use devcon_proto::protos::StartSocket;
use devcon_proto::protos::StopSocket;
use protobuf::Serialize;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    host: String,

    port: i32,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Opens url in host browser
    Browser {
        /// Url to be opened
        #[arg(short, long)]
        url: String,
    },

    StartSocket {
        #[arg(short, long)]
        port: u32,
    },

    StopSocket {
        #[arg(short, long)]
        port: u32,
    },
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let remote = format!("{}:{}", cli.host, cli.port);

    let mut stream = TcpStream::connect(remote)?;
    match cli.command {
        Commands::Browser { url } => {
            let mut browser = Browser::new();
            let mut message = Request::new();

            browser.set_url(url);
            message.set_browser(browser);

            stream.send(&message.serialize().unwrap())?;
        }
        Commands::StartSocket { port } => {
            let listener = TcpListener::bind("0.0.0.0:0")?;

            let mut message = Request::new();
            let mut command = StartSocket::new();

            command.set_port_number(port);
            command.set_peer_address(listener.local_addr().expect("expect").to_string());
            message.set_start_socket(command);

            stream.send(&message.serialize().unwrap())?;

            loop {
                let new_incoming_connection = listener.accept();
                if let Ok(new_connection) = new_incoming_connection {
                    let new_connection = new_connection.0;

                    // Launch new thread
                    let builder = thread::Builder::new().name(
                        new_connection
                            .peer_addr()
                            .expect("Should get peer information")
                            .to_string(),
                    );
                    builder
                        .spawn(move || handle_forward(new_connection, port))
                        .expect("Couldn't create a thread.");
                } else {
                    eprintln!("Couldn't establish incoming connection.");
                }
            }
        }
        Commands::StopSocket { port } => {
            let mut command = StopSocket::new();
            let mut message = Request::new();

            command.set_port_number(port);
            message.set_stop_socket(command);

            stream.send(&message.serialize().unwrap())?;
        }
    }
    Ok(())
}

fn handle_forward(mut new_connection: TcpStream, port: u32) -> () {
    let mut local_peer = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("fail");
    loop {
        let mut buffer = vec![0; 512];
        let result = new_connection.read(&mut buffer);

        match result {
            Ok(_) => {
                local_peer.write_all(&buffer).expect("fail");
            }
            _ => {
                eprintln!("Could not read buffer");
                break;
            }
        }
    }
    local_peer.shutdown(std::net::Shutdown::Both).expect("fail");
}
