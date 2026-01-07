
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};

use serde_json::Value;

use crate::{
    SOCKET_PATH,
    devcontainer::{Devcontainer, Feature},
    driver,
};

pub fn start_server() -> anyhow::Result<()> {
    let socket_path = Path::new(SOCKET_PATH);
    if socket_path.exists() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(e) = handle_client(&mut stream) {
                    eprintln!("Error handling client: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }

    Ok(())
}

pub fn handle_client(stream: &mut UnixStream) -> anyhow::Result<()> {
    let mut buffer = [0u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;

    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let parts: Vec<&str> = request.trim().split('\n').collect();

    if parts.is_empty() {
        return Ok(());
    }

    let response = match parts[0] {
        "UP" => {
            let path = parts.get(1).unwrap_or(&".");
            handle_up_request(path)
        }
        "SHELL" => {
            let path = parts.get(1).unwrap_or(&".");
            let env: Vec<String> = parts.iter().skip(2).map(|s| s.to_string()).collect();
            handle_shell_request(path, &env)
        }
        _ => {
            format!("ERROR: Unknown command")
        }
    };

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn handle_up_request(path: &str) -> String {
    let devcontainer = Devcontainer {
        name: "default".to_string(),
        image: "ubuntu".to_string(),
        features: vec![Feature {
            url: "ghcr.io/devcontainers/features/github-cli:1".to_string(),
            options: Value::Null,
        }],
    };

    let result = driver::container::build(&devcontainer);
    let _ = driver::container::start(&devcontainer, path);

    if result.is_ok() {
        format!("OK: Starting devcontainer at path: {}", path)
    } else {
        format!("ERR: Starting devcontainer at path: {}", path)
    }
}

fn handle_shell_request(path: &str, env: &[String]) -> String {
    let env_str = if env.is_empty() {
        "none".to_string()
    } else {
        env.join(", ")
    };
    format!("OK: Opening shell at path: {} with env: {}", path, env_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_handle_up_request() {
        let response = handle_up_request("/test/path");
        assert_eq!(response, "OK: Starting devcontainer at path: /test/path");
    }

    #[test]
    fn test_handle_up_request_current_dir() {
        let response = handle_up_request(".");
        assert_eq!(response, "OK: Starting devcontainer at path: .");
    }

    #[test]
    fn test_handle_shell_request_no_env() {
        let env: Vec<String> = vec![];
        let response = handle_shell_request("/test/path", &env);
        assert_eq!(
            response,
            "OK: Opening shell at path: /test/path with env: none"
        );
    }

    #[test]
    fn test_handle_shell_request_with_env() {
        let env = vec!["KEY1=VALUE1".to_string(), "KEY2=VALUE2".to_string()];
        let response = handle_shell_request("/test/path", &env);
        assert_eq!(
            response,
            "OK: Opening shell at path: /test/path with env: KEY1=VALUE1, KEY2=VALUE2"
        );
    }

    #[test]
    fn test_handle_client_up_command() {
        let socket_path = "/tmp/devcon_test_up.sock";
        let _ = fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                handle_client(&mut stream).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = UnixStream::connect(socket_path).unwrap();
        client.write_all(b"UP\n/my/project").unwrap();
        client.flush().unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert_eq!(response, "OK: Starting devcontainer at path: /my/project");

        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn test_handle_client_shell_command() {
        let socket_path = "/tmp/devcon_test_shell.sock";
        let _ = fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                handle_client(&mut stream).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = UnixStream::connect(socket_path).unwrap();
        client.write_all(b"SHELL\n/my/project\nKEY=VALUE").unwrap();
        client.flush().unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert_eq!(
            response,
            "OK: Opening shell at path: /my/project with env: KEY=VALUE"
        );

        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn test_handle_client_unknown_command() {
        let socket_path = "/tmp/devcon_test_unknown.sock";
        let _ = fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                handle_client(&mut stream).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut client = UnixStream::connect(socket_path).unwrap();
        client.write_all(b"INVALID\n/path").unwrap();
        client.flush().unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert_eq!(response, "ERROR: Unknown command");

        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn test_handle_client_empty_request() {
        let socket_path = "/tmp/devcon_test_empty.sock";
        let _ = fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path).unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let result = handle_client(&mut stream);
                assert!(result.is_ok());
            }
        });

        thread::sleep(Duration::from_millis(100));

        let client = UnixStream::connect(socket_path).unwrap();
        drop(client); // Close connection immediately

        thread::sleep(Duration::from_millis(100));

        let _ = fs::remove_file(socket_path);
    }
}
