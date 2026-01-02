use std::net::TcpStream;

use clap::Parser;
use clap::Subcommand;
use devcon_proto::TcpWithSize;
use devcon_proto::protos::Browser;
use devcon_proto::protos::Request;
use protobuf::Serialize;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    host: String,

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
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let mut stream = TcpStream::connect(cli.host)?;
    match cli.command {
        Commands::Browser { url } => {
            let mut browser = Browser::new();
            browser.set_url(url);
            let mut message = Request::new();
            message.set_browser(browser);

            let out = message.serialize().unwrap();

            stream.send(&out)?;
        }
    }
    Ok(())
}
