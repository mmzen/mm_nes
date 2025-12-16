#!/bin/bash
# Authorship: Human 0% | Claude 100%
#
# Script to install code quality metrics tools for CI
# Used by bitbucket-pipelines.yml

set -e

echo "Installing code quality metrics tools..."

# Install rust-code-analysis-cli (pre-built binary - faster than compiling)
RCA_VERSION="0.0.25"
ARCH=$(uname -m)

if [ "$ARCH" = "x86_64" ]; then
    RCA_BINARY="rust-code-analysis-linux-cli-x86_64.tar.gz"
elif [ "$ARCH" = "aarch64" ]; then
    RCA_BINARY="rust-code-analysis-linux-cli-aarch64.tar.gz"
else
    echo "Unsupported architecture: $ARCH, falling back to cargo install"
    cargo install rust-code-analysis-cli --locked
    RCA_BINARY=""
fi

if [ -n "$RCA_BINARY" ]; then
    wget -q "https://github.com/mozilla/rust-code-analysis/releases/download/v${RCA_VERSION}/${RCA_BINARY}"
    tar -xzf "$RCA_BINARY"
    mv rust-code-analysis-cli /usr/local/bin/
    chmod +x /usr/local/bin/rust-code-analysis-cli
    rm -f "$RCA_BINARY"
fi

echo "rust-code-analysis-cli installed: $(rust-code-analysis-cli --version)"

# Install cargo-tarpaulin for coverage (only if requested)
if [ "$1" = "--with-coverage" ]; then
    echo "Installing cargo-tarpaulin for coverage..."
    cargo install cargo-tarpaulin --locked
    echo "cargo-tarpaulin installed"
fi

# Install Python dependencies
echo "Installing Python dependencies..."
pip3 install --quiet -r metrics/requirements.txt

echo "Tools installed successfully"
