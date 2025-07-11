# DevCon Browser Integration

This feature allows you to open URLs in the host's default browser from within a devcontainer.

## How it works

1. **Socket Server**: DevCon can run a socket server on the host that listens for URL requests
2. **Socket Mount**: When starting a devcontainer, the socket is automatically mounted inside the container
3. **Browser Helper**: A helper script inside the container can send URLs to the socket server
4. **URL Opening**: The socket server receives the URL and opens it in the host's default browser

## Usage

### 1. Start the socket server

On the host machine, start the socket server:

```bash
# Start in foreground (for testing)
devcon socket

# Start in daemon mode (background)
devcon socket --daemon

# Use custom socket path
devcon socket --socket-path /custom/path/browser.sock
```

### 2. Start your devcontainer

The socket will be automatically mounted when you start a devcontainer:

```bash
devcon open /path/to/your/project
```

### 3. Open URLs from inside the container

Inside the devcontainer, you can use the provided helper script:

```bash
# Copy the helper script to the container (if not already there)
cp /workspaces/devcon/scripts/devcon-browser.sh /usr/local/bin/devcon-browser
chmod +x /usr/local/bin/devcon-browser

# Open a URL
devcon-browser https://github.com
devcon-browser https://localhost:3000
```

Alternatively, you can send URLs directly to the socket:

```bash
echo "https://github.com" | nc -U /tmp/devcon-browser.sock
```

## Socket Location

- **Host**: `~/.devcon/browser.sock` (default)
- **Container**: `/tmp/devcon-browser.sock`

## Requirements

- Unix-like system (Linux, macOS) - Windows support via WSL
- `nc` (netcat) command available in the container for the helper script
- Default browser configured on the host system

## Security Considerations

- The socket is created with permissions `0660` (owner and group read/write)
- Only processes running as the same user or group can access the socket
- URLs are not validated - ensure you trust the source sending URLs to the socket

## Troubleshooting

### Socket not found
- Ensure the socket server is running: `devcon socket --daemon`
- Check if the socket file exists: `ls -la ~/.devcon/browser.sock`

### Permission denied
- Check socket permissions: `ls -la ~/.devcon/browser.sock`
- Ensure your user has access to the socket file

### Browser not opening
- Check if a default browser is configured on the host
- Try running the socket server in foreground mode to see error messages
- Verify the URL format is correct (must include protocol: `https://` or `http://`)

## Examples

```bash
# Start socket server in daemon mode
devcon socket --daemon

# Start devcontainer (socket will be mounted automatically)
devcon open ~/my-project

# Inside container - open development server
devcon-browser http://localhost:3000

# Inside container - open documentation
devcon-browser https://docs.example.com

# Inside container - open GitHub repository
devcon-browser https://github.com/user/repo
```
