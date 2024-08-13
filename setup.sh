#!/bin/bash

echo "Script executed from: ${PWD}"
# save current directory
BASEDIR=$PWD
git submodule update --init
sudo apt-get update -y
sudo apt upgrade -y
sudo apt install make gcc git apt-transport-https ca-certificates curl software-properties-common -y
sudo apt-get install pkg-config libavutil-dev libavformat-dev libavcodec-dev libavdevice-dev -y # for ffmpeg
sudo apt-get install libclang-dev clang -y # for ffmpeg
# Install Rust, pnpm, nvm, node, docker
curl https://sh.rustup.rs -sSf | sh -s -- -y
wget -qO- https://get.pnpm.io/install.sh | sh -
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
export NVM_DIR="$([ -z "${XDG_CONFIG_HOME-}" ] && printf %s "${HOME}/.nvm" || printf %s "${XDG_CONFIG_HOME}/nvm")"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # This loads nvm
nvm install 20
node -v
npm -v
sudo apt-get install nodejs -y

# Configure current shell to include Cargo's bin directory
source ~/.bashrc
. "$HOME/.cargo/env"

cargo install loco-cli
cargo install sea-orm-cli

export PATH="$PATH:$HOME/.local/share/pnpm"
# Verify pnpm installation and add pnpm to PATH immediately
if command -v pnpm >/dev/null; then
    echo "pnpm has been successfully installed and is available."
else
    echo "There was a problem adding pnpm to the PATH. Please check the installation."
fi

cd $BASEDIR/frontend
pnpm install
pnpm build:production
cd $BASEDIR

# Install Docker if we are not in wsl
if [ ! -f "/proc/sys/fs/binfmt_misc/WSLInterop" ]; then
    sudo exec bash docker_install.sh
fi
if [-f "/proc/sys/fs/binfmt_misc/WSLInterop" ]; then
    echo "WSL detected, skipping Docker installation"
    echo "Please install Docker manually in your Windows machine and setup WSL2 integration"
fi
docker volume create pgdata
docker run -d -p 5432:5432 -e POSTGRES_USER=loco -e POSTGRES_DB=connect_development -e POSTGRES_PASSWORD="loco" -v pgdata:/var/lib/postgresql/data postgres:15.3-alpine
docker run -p 6379:6379 -d redis redis-server
# echo current directory

# git clone --depth 1 --branch master https://github.com/Moretore/openpilot.git
# cd openpilot
# git submodule update --init --recursive --depth 1
# # build openpilot with docker
# docker build -t openpilot -f Dockerfile.openpilot .
# or if we need to use custom version of openpilot we need to rebuild it locallly and then use FROM openpilot-base:local
# docker build -t openpilot:local -f Dockerfile.openpilot_base .
# modify Dockerfile.openpilot to use FROM openpilot:local instead of FROM ghcr.io/commaai/openpilot-base:latest
cd $BASEDIR 
cd minikeyvalue
docker volume create kvstore
docker build -t minikeyvalue -f Dockerfile .
docker run -d -p 3000-3005:3000-3005 -v kvstore:/tmp minikeyvalue
cd $BASEDIR
git clone --depth 1 https://github.com/commaai/cereal.git
go version
rustc --version
pnpm -v
node -v
docker -v
docker-compose -v
cargo -V
cd $BASEDIR
pip install -U "huggingface_hub[cli,hf_transfer]"

