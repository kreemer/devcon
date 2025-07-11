# DevCon - Development Container Manager

```
██████╗ ███████╗██╗   ██╗ ██████╗ ██████╗ ███╗   ██╗
██╔══██╗██╔════╝██║   ██║██╔════╝██╔═══██╗████╗  ██║
██║  ██║█████╗  ██║   ██║██║     ██║   ██║██╔██╗ ██║
██║  ██║██╔══╝  ╚██╗ ██╔╝██║     ██║   ██║██║╚██╗██║
██████╔╝███████╗ ╚████╔╝ ╚██████╗╚██████╔╝██║ ╚████║
╚═════╝ ╚══════╝  ╚═══╝   ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝
                                                     
    "DevCon One" - Your Mission-Critical Dev Environment Manager
```

A blazingly fast TUI application for managing and launching development containers with ease. 

## ✨ Features

- 🚀 **Quick Launch**: Instantly spin up development containers with the DevContainer CLI
- 📝 **Recent Projects**: Keep track of your frequently used projects with an intuitive TUI
- 🧩 **Additional Features**: Configure and manage additional devcontainer features
- ⚡ **Lightning Fast**: Built with Rust for optimal performance

## 🛠 Installation

### From Source

```bash
git clone https://github.com/kreemer/devcon.git
cd devcon
cargo install --path .
```

### From Releases

Download the latest binary from the [releases page](https://github.com/kreemer/devcon/releases).

### Prerequisites

DevCon requires the [DevContainer CLI](https://github.com/devcontainers/cli) to be installed:

```bash
npm install -g @devcontainers/cli
```

## 🚀 Quick Start

### Check System Requirements

```bash
devcon check
```

### Launch TUI Mode

```bash
devcon
```

Navigate with `↑/↓` or `k/j`, select with `Enter`, quit with `q/Esc`.

### Open a Project Directly

```bash
devcon open /path/to/your/project
```

### Configure Dotfiles Repository

```bash
devcon config dotfiles set https://github.com/kreemer/dotfiles
devcon config dotfiles show
```

### Manage Additional Features

```bash
# Add features
devcon config features add ghcr.io/devcontainers/features/github-cli:1 '{}'
devcon config features add ghcr.io/devcontainers/features/docker-in-docker:2 '{ "version": "none" }'

# List configured features
devcon config features list

# Remove a feature
devcon config features remove ghcr.io/devcontainers/features/github-cli:1

# Clear all features
devcon config features clear
```

## 📖 Usage

### Commands

| Command | Description |
|---------|-------------|
| `devcon` | Launch TUI mode to select from recent projects |
| `devcon open [PATH]` | Open a development container for the specified path |
| `devcon shell [PATH]` | Execute a shell in a development container |
| `devcon check` | Verify DevContainer CLI installation |
| `devcon socket [--daemon]` | Start socket server for browser integration |
| `devcon socket --show-path` | Show the default socket path location |
| `devcon config dotfiles set <URL>` | Set dotfiles repository URL |
| `devcon config dotfiles show` | Show current dotfiles configuration |
| `devcon config dotfiles clear` | Remove dotfiles configuration |
| `devcon config features add <FEATURE> <VALUE>` | Add an additional feature |
| `devcon config features list` | List all configured features |
| `devcon config features remove <FEATURE>` | Remove a feature |
| `devcon config features clear` | Clear all features |

### Configuration

DevCon stores its configuration in your XDG config directory (usually `~/.config/devcon/config.yaml`).

Example configuration:
```yaml
recent_paths:
  - /home/user/projects/awesome-app
  - /home/user/projects/web-service
dotfiles_repo: https://github.com/user/dotfiles
additional_features:
  ghcr.io/devcontainers/features/github-cli:1: latest
  ghcr.io/devcontainers/features/docker-in-docker:2: "20.10"
```

### Browser Integration

DevCon includes a seamless browser integration feature that allows you to open URLs in your host's default browser from within a devcontainer.

#### Quick Start

1. **Start the socket server** (on your host):
   ```bash
   devcon socket --daemon
   ```

2. **Start your devcontainer** (socket and helper script are automatically mounted):
   ```bash
   devcon open /path/to/your/project
   ```

3. **Open URLs from inside the container** (helper script is automatically available):
   ```bash
   # The helper script is automatically mounted at /usr/local/bin/devcon-browser
   devcon-browser https://github.com
   devcon-browser http://localhost:3000
   
   # Or use the BROWSER environment variable (automatically set in exec shells)
   devcon shell /path/to/your/project
   # Inside the container:
   $BROWSER https://github.com
   
   # Find your socket path on the host
   devcon socket --show-path
   ```

For detailed information, see [Browser Integration Guide](docs/BROWSER_INTEGRATION.md).

## 🏗 Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy
cargo fmt
```

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [DevContainers](https://containers.dev/) for the amazing development container specification
- [Ratatui](https://github.com/ratatui-org/ratatui) for the excellent TUI framework
- [Clap](https://github.com/clap-rs/clap) for the CLI argument parsing

---

> "DevCon One is ready for deployment!" 🚀
