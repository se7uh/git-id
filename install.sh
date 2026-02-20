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
      arm64)   ASSET="git-id-aarch64-macos" ;;
      *)       echo "Unsupported architecture: $ARCH (only Apple Silicon is supported)" >&2; exit 1 ;;
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

# Determine SHA256 checksum URL
CHECKSUM_URL="${DOWNLOAD_URL}.sha256"

# Create install directory if it doesn't exist
mkdir -p "$INSTALL_DIR"

# Download binary to a temporary file first to avoid leaving a corrupted binary on failure
echo "Downloading $ASSET..."
TMP_FILE="$(mktemp "$INSTALL_DIR/git-id.XXXXXX")" || { echo "Failed to create temporary file" >&2; exit 1; }
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE"
  CHECKSUM_LINE="$(curl -fsSL "$CHECKSUM_URL")" || { rm -f "$TMP_FILE"; echo "Failed to download checksum file" >&2; exit 1; }
else
  wget -qO "$TMP_FILE" "$DOWNLOAD_URL"
  CHECKSUM_LINE="$(wget -qO- "$CHECKSUM_URL")" || { rm -f "$TMP_FILE"; echo "Failed to download checksum file" >&2; exit 1; }
fi

# Verify SHA256 checksum
EXPECTED_HASH="$(echo "$CHECKSUM_LINE" | awk '{print $1}')"
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_HASH="$(sha256sum "$TMP_FILE" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL_HASH="$(shasum -a 256 "$TMP_FILE" | awk '{print $1}')"
else
  echo "Warning: sha256sum/shasum not found, skipping checksum verification" >&2
  ACTUAL_HASH="$EXPECTED_HASH"
fi

if [ "$ACTUAL_HASH" != "$EXPECTED_HASH" ]; then
  rm -f "$TMP_FILE"
  echo "Checksum verification failed!" >&2
  echo "  Expected: $EXPECTED_HASH" >&2
  echo "  Got:      $ACTUAL_HASH" >&2
  exit 1
fi

mv "$TMP_FILE" "$INSTALL_DIR/git-id"
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
