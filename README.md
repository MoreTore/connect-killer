This is an open-source alternative to openpilot connect for use with openpilot software.

To get started, inspect the docker compose yaml to adjsut the volume mount points.
https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/docker-compose.yml#L3-L53
fill out the .env_template and rename it to .env
https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/.env_template#L1-L18
create openssl keys for your domain and put them into self_signed_certs folder. See here https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/src/app.rs#L151-L158

run docker compose up --build

