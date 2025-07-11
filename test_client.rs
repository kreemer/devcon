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

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <url>", args[0]);
        eprintln!("Example: {} https://github.com", args[0]);
        return Ok(());
    }
    
    let url = &args[1];
    let socket_path = PathBuf::from("/tmp/devcon-browser.sock");
    
    // For testing, use the host socket path
    let socket_path = PathBuf::from("/home/vscode/.devcon/browser.sock");
    
    if !socket_path.exists() {
        eprintln!("Error: Socket not found at {}", socket_path.display());
        eprintln!("Make sure the devcon socket server is running:");
        eprintln!("  devcon socket --daemon");
        return Ok(());
    }
    
    let mut stream = UnixStream::connect(&socket_path)?;
    
    // Send the URL
    writeln!(stream, "{}", url)?;
    
    // Read the response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    
    println!("Response: {}", response.trim());
    
    Ok(())
}
