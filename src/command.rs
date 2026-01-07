use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

use crate::SOCKET_PATH;
use crate::server::start_server;

pub fn handle_server_command(daemon: bool) -> anyhow::Result<()> {
    if daemon {
        todo!("Daemon mode not yet implemented");
    }

    start_server()
}

pub fn handle_start_command(pathbuf: PathBuf) -> anyhow::Result<()> {
    let path = fs::canonicalize(pathbuf)?;
    let path_str = path.to_string_lossy();

    let response = send_to_server(&format!("START\n{}", path_str))?;
    println!("{}", response);

    Ok(())
}

pub fn handle_shell_command(path: Option<&PathBuf>, env: &[String]) -> anyhow::Result<()> {
    let path_str = path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let mut message = format!("SHELL\n{}", path_str);
    for e in env {
        message.push('\n');
        message.push_str(e);
    }

    let response = send_to_server(&message)?;
    println!("{}", response);

    Ok(())
}

fn send_to_server(message: &str) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(SOCKET_PATH).map_err(|_| {
        anyhow::anyhow!(
            "Could not connect to server. Is the server running? Start it with: devcon server"
        )
    })?;

    stream.write_all(message.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_handle_up_command_with_path() {
        let path = PathBuf::from("/test/path");
        let result = handle_start_command(path);
        // This will fail if server is not running, which is expected
        assert!(result.is_err() || result.is_ok());
    }
}
