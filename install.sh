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
  checksums_url="https://github.com/${repo}/releases/latest/download/SHA256SUMS"
else
  url="https://github.com/${repo}/releases/download/${version}/${asset}"
  checksums_url="https://github.com/${repo}/releases/download/${version}/SHA256SUMS"
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

echo "Downloading ${asset}..."
curl -fL "$url" -o "$tmp_dir/$asset"
curl -fsSL "$checksums_url" -o "$tmp_dir/SHA256SUMS"
expected="$(awk -v asset="$asset" '$2 == asset { print $1 }' "$tmp_dir/SHA256SUMS")"
if [ -z "$expected" ]; then
  echo "No checksum was published for $asset" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp_dir/$asset" | awk '{ print $1 }')"
else
  actual="$(shasum -a 256 "$tmp_dir/$asset" | awk '{ print $1 }')"
fi
if [ "$actual" != "$expected" ]; then
  echo "Checksum verification failed for $asset" >&2
  exit 1
fi
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
mkdir -p "$install_dir"
install -m 0755 "$tmp_dir/expander-mcp" "$install_dir/expander-mcp"

echo "Installed $install_dir/expander-mcp"
"$install_dir/expander-mcp" setup "$@"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) echo "Add $install_dir to PATH to use expander-mcp from your shell." ;;
esac
