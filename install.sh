#!/bin/sh
set -e

REPO="se7uh/git-id"
INSTALL_DIR="$HOME/.local/bin"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64)  ASSET="git-id-x86_64-linux" ;;
      aarch64) ASSET="git-id-aarch64-linux" ;;
      *)       echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      x86_64)  ASSET="git-id-x86_64-macos" ;;
      arm64)   ASSET="git-id-aarch64-macos" ;;
      *)       echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# Fetch the latest release download URL
API_URL="https://api.github.com/repos/$REPO/releases/latest"
if command -v curl >/dev/null 2>&1; then
  if command -v jq >/dev/null 2>&1; then
    DOWNLOAD_URL="$(curl -fsSL "$API_URL" | jq -r --arg name "$ASSET" '.assets[] | select(.name == $name) | .browser_download_url' | head -n 1)"
  else
    DOWNLOAD_URL="$(curl -fsSL "$API_URL" | grep '"browser_download_url"' | grep "$ASSET" | sed -n 's/.*"browser_download_url":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  fi
elif command -v wget >/dev/null 2>&1; then
  if command -v jq >/dev/null 2>&1; then
    DOWNLOAD_URL="$(wget -qO- "$API_URL" | jq -r --arg name "$ASSET" '.assets[] | select(.name == $name) | .browser_download_url' | head -n 1)"
  else
    DOWNLOAD_URL="$(wget -qO- "$API_URL" | grep '"browser_download_url"' | grep "$ASSET" | sed -n 's/.*"browser_download_url":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  fi
else
  echo "curl or wget is required" >&2
  exit 1
fi

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find download URL for $ASSET" >&2
  exit 1
fi

# Create install directory if it doesn't exist
mkdir -p "$INSTALL_DIR"

# Download binary
echo "Downloading $ASSET..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$INSTALL_DIR/git-id"
else
  wget -qO "$INSTALL_DIR/git-id" "$DOWNLOAD_URL"
fi

chmod +x "$INSTALL_DIR/git-id"

echo ""
echo "git-id installed to $INSTALL_DIR/git-id"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH."
echo ""
echo "To enable shell completions, run:"
echo "  git-id completions bash   # for Bash"
echo "  git-id completions zsh    # for Zsh"
echo "  git-id completions fish   # for Fish"
