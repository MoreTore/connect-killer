This is an open-source alternative to openpilot connect for use with openpilot software.
To see the server in action, go to: https://stable.konik.ai/

Thank you https://konik.ai for hosting!

# To make your device connect to the server, complete the following steps:
Note. There is no need to unpair the device from comma connect.

* Step 1: SSH into the device.
* Step 2 (Cloned comma devices only): Make sure you generate unique OpenSSL key pairs on the device. You can copy a script from here https://github.com/1okko/openpilot/blob/mr.one/1.sh to generate the keys.

* Step 3: Delete the device dongle ID by running rm /data/params/d/DongleId and rm /persist/comma/dongle_id

If you are running a custom fork of openpilot that already has the code changes required, then you can reboot the device now and scan the qr code on the [website](https://stable.konik.ai/) pair the device.

If you are using a fork that does not have the code changes, you will need to continue with the following steps:

Step 4: export the server urls in launch_openpilot.sh by adding this to that file.
```bash
#!/usr/bin/bash
export API_HOST=https://api.konik.ai
export ATHENA_HOST=wss://athena.konik.ai
# Any other custom launch options here
exec ./launch_chffrplus.sh
```

Step 5: Commit your changes and disable automatic software updates in the openpilot settings (if applicable).
```git commit -a -m "switched to konik server"```

Step 5: Reboot the device and scan the QR code on the [website](https://stable.konik.com/). The QR code must be scanned with the konik website and not comma connect.


# Hosting you own instance (Hardcore)

To get started with hosting your own instance, inspect the docker compose yaml to adjust the volume mount points.
https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/docker-compose.yml#L3-L53
fill out the .env_template and rename it to .env
https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/.env_template#L1-L18
create openssl keys for your domain and put them into self_signed_certs folder. See here https://github.com/MoreTore/connect-killer/blob/4b9be8252688df5672448b1139da4b4a71c554dc/src/app.rs#L151-L158
More changes to hard coded values need to be changed to get the frontend working. More work needs to be done to make it easier.

run docker compose up --build
