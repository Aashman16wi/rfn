#!/usr/bin/env bash
# rfn installation script
# Usage: ./install.sh [--features "image-preview,pdf-preview,archive-preview"]

set -euo pipefail

FEATURES="${1:-image-preview,pdf-preview,archive-preview}"
BINARY_NAME="rfn"
INSTALL_DIR="${HOME}/.local/bin"

echo "Building rfn with features: ${FEATURES}..."
cargo build --release --features "${FEATURES}"

echo "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
install -Dm755 "target/release/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

# Check if install dir is in PATH
if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
    echo "⚠️  ${INSTALL_DIR} is not in your PATH"
    echo "Add this to your shell config (.bashrc, .zshrc, etc.):"
    echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi

echo "✅ rfn installed to ${INSTALL_DIR}/${BINARY_NAME}"
echo "Run 'rfn --help' to get started"