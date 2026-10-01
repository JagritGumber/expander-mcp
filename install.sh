#!/bin/sh
set -eu

repo="JagritGumber/expander-mcp"
install_dir="${EXPANDER_INSTALL_DIR:-$HOME/.local/bin}"
version="${EXPANDER_VERSION:-latest}"

case "$(uname -s)" in
  Linux) os="linux" ;;
  Darwin) os="macos" ;;
  *) echo "Unsupported operating system. Use a release binary from GitHub." >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

asset="expander-mcp-${os}-${arch}.tar.gz"
if [ "$version" = "latest" ]; then
  url="https://github.com/${repo}/releases/latest/download/${asset}"
else
  url="https://github.com/${repo}/releases/download/${version}/${asset}"
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

echo "Downloading ${asset}..."
curl -fL "$url" -o "$tmp_dir/$asset"
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
mkdir -p "$install_dir"
install -m 0755 "$tmp_dir/expander-mcp" "$install_dir/expander-mcp"

echo "Installed $install_dir/expander-mcp"
"$install_dir/expander-mcp" setup

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "Add $install_dir to PATH to use expander-mcp from your shell." ;;
esac

