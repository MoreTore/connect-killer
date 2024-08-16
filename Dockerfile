# Use Ubuntu 20.04 as the base image
FROM ubuntu:20.04

# Set the environment to noninteractive to avoid prompts
ENV DEBIAN_FRONTEND=noninteractive

# Update and install dependencies in one RUN command
RUN apt-get update && apt-get install -y \
    build-essential \
    clang \
    curl \
    wget \
    libssl-dev \
    pkg-config \
    libavutil-dev \
    libavformat-dev \
    libavcodec-dev \
    libavdevice-dev \
    capnproto \
    nodejs \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages
RUN pip3 install -U "huggingface_hub[cli,hf_transfer]"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- --default-toolchain 1.80.1 -y

# Ensure that the Rust environment is available in the current session
ENV PATH="/root/.cargo/bin:${PATH}"
# Set up a Cargo cache directory
ENV CARGO_HOME=/usr/local/cargo
RUN mkdir -p $CARGO_HOME
VOLUME $CARGO_HOME

# Set the working directory
WORKDIR /usr/src/connect

# Copy the Cargo.toml and source code
COPY . .

# Load NVM and install Node.js
RUN /bin/bash -c "./install_deps.sh"
# Build the application with necessary features
RUN /bin/bash -c "source $HOME/.cargo/env && cargo install loco-cli cargo-insta sea-orm-cli"
RUN /bin/bash -c "source $HOME/.cargo/env && cargo build"

# Expose the ports your server runs on
# HTTPS
EXPOSE 3222
EXPOSE 3223
# HTTP
EXPOSE 3111
EXPOSE 3112


CMD ./start_useradmin.sh & ./start_connect.sh & wait