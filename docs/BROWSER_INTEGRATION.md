# DevCon Browser Integration

This feature allows you to open URLs in the host's default browser from within a devcontainer with seamless integration.

## How it works

1. **Socket Server**: DevCon runs a socket server on the host that listens for URL requests
2. **Automatic Setup**: When starting a devcontainer, DevCon automatically:
   - Mounts the socket inside the container
   - Creates and mounts a helper script at `/usr/local/bin/devcon-browser`
   - Sets the `BROWSER` environment variable in exec shells
3. **URL Opening**: Applications inside the container can send URLs to the socket server
4. **Host Integration**: The socket server receives URLs and opens them in the host's default browser

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

The helper script is automatically available inside the container:

```bash
# Direct usage
devcon-browser https://github.com
devcon-browser https://localhost:3000

# Using the BROWSER environment variable (available in exec shells)
devcon shell /path/to/your/project
# Inside the container:
$BROWSER https://github.com

# Or with environment expansion
export URL="https://github.com"
$BROWSER "$URL"
```

Alternatively, you can send URLs directly to the socket:

```bash
echo "https://github.com" | nc -U /tmp/devcon-browser.sock
```

## Automatic Integration

When you start a devcontainer with an active socket server, DevCon automatically:

- **Mounts the socket**: Host socket → `/tmp/devcon-browser.sock` in container
- **Mounts helper script**: Auto-generated script → `/usr/local/bin/devcon-browser` in container
- **Sets BROWSER env**: In exec shells, `BROWSER=devcon-browser` is automatically set

## Socket Location

- **Host**: XDG runtime directory (typically `$XDG_RUNTIME_DIR/devcon/browser.sock` or `~/.local/share/devcon/browser.sock`)
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
- Check if the socket file exists in your XDG runtime directory

### Permission denied
- Check socket permissions in your XDG runtime directory
- Ensure your user has access to the socket file

### Browser not opening
- Check if a default browser is configured on the host
- Try running the socket server in foreground mode to see error messages
- Verify the URL format is correct (must include protocol: `https://` or `http://`)

## Examples

```bash
# Start socket server in daemon mode
devcon socket --daemon

# Start devcontainer (socket and helper automatically mounted)
devcon open ~/my-project

# Inside container - helper script is ready to use
devcon-browser http://localhost:3000

# Inside container - open documentation
devcon-browser https://docs.example.com

# Using exec shell with BROWSER environment variable
devcon shell ~/my-project
# Inside shell:
$BROWSER https://github.com/user/repo

# Check socket location on host
devcon socket --show-path
```
