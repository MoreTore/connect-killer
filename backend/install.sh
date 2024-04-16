#!/bin/bash -e
sudo apt update
sudo apt install nginx
sudo apt install golang-go
go version

# get root directory
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
echo $DIR

cd $DIR && cd .. 
git submodule update --init --recursive
cd minikeyvalue/src
go build -o mkv

cd ..
pip install -r requirements.txt


