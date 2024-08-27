import requests
import os
import time
import json

base_url = os.environ['API_ENDPOINT']
assert base_url, "API_ENDPOINT' not set or is empty"
authorization = os.environ['ADMIN_JWT'] # get jwt from https://connect-portal.duckdns.org browser debug tools. stored in app storage
assert authorization, "ADMIN_JWT not set or is empty"

# Get all devices.
def get_devices():
    url = f'{base_url}/v1/me/devices?sig={authorization}'
    response = requests.get(url)
    
    if response.status_code == 200:
        devices = response.json()
        return devices
    else:
        print({"error": response.status_code, "message": response.text})
        return None

def find_routes(dongle_id):
    now = int(time.time())*1000
    start_time = (int(time.time())-86400)*1000 # 1 day ago in ms
    url = f'{base_url}/v1/devices/{dongle_id}/routes_segments?start={start_time}&end={now}&sig={authorization}'
    response = requests.get(url)
    if response.status_code == 200:
        routes = response.json()
        return routes
    else:
        print({"error": response.status_code, "message": response.text})
        return None

def get_route_files(route_name):
    url = f'{base_url}/v1/route/{route_name}/files?sig={authorization}'
    response = requests.get(url)
    if response.status_code == 200:
        files = response.json()
        return files
    else:
        print({"error": response.status_code, "message": response.text})
        return None

def ls_log_dir(dongle_id):
    jsonrpc_request = {
        "jsonrpc": "2.0",
        "method": "listDataDirectory",
        "id": 0
    }
    headers = {
        "Authorization": f"JWT {authorization}",
        "Content-Type": "application/json"
    }
    try:
        response = requests.post(f'{base_url}/ws/{dongle_id}', headers=headers, data=json.dumps(jsonrpc_request), timeout=5)
    except requests.exceptions.Timeout:
        print(f"Timeout occurred while getting log dir for {dongle_id}")
        return None
    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")
        return None
    if response.status_code == 200:
        json_response = response.json()
        if isinstance(json_response.get('result'), list):
            return json_response.get('result')
        else:
            print("Error")
    else:
        print(f"Failed to send request: {response.status_code}")
        print("Response:", response.text)

def get_upload_queue(dongle_id):
    jsonrpc_request = {
        "jsonrpc": "2.0",
        "method": "listUploadQueue",
        "id": 0
    }
    headers = {
        "Authorization": f"JWT {authorization}",
        "Content-Type": "application/json"
    }
    try:
        response = requests.post(f'{base_url}/ws/{dongle_id}', headers=headers, data=json.dumps(jsonrpc_request), timeout=15)
    except requests.exceptions.Timeout:
        print(f"Timeout occurred while getting upload queue for {dongle_id}")
        return None
    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")
        return None
    if response.status_code == 200:
        json_response = response.json()
        if isinstance(json_response.get('result'), list):
            print(f"{dongle_id} is uploading files")
            return json_response.get('result')
        else:
            print("Error")
    else:
        print(f"Failed to send request: {response.status_code}")
        print("Response:", response.text)

def get_upload_urls(dongle_id, paths):
    url = f'{base_url}/v1/{dongle_id}/upload_urls'
    for i, path in enumerate(paths):
        if 'rlog' in path:
            paths[i] = path + ".bz2"
        if 'qlog' in path:
            paths[i] = path + ".bz2"
        if 'qcam' in path:
            paths[i] = path + ".ts"
        if 'fcam' in path:
            paths[i] = path + ".hevc"
        if 'dcam' in path:
            paths[i] = path + ".hevc"
        if 'ecam' in path:
            paths[i] = path + ".hevc"

    payload = {
        "paths": paths,
    }
    headers = {
        "Authorization": f"JWT {authorization}",
        "Content-Type": "application/json"
    }
    response = requests.post(url, headers=headers, json=payload)
    if response.status_code == 200:
        upload_urls = response.json()

        return upload_urls
    else:
        print()
        return {"error": response.status_code, "message": response.text}
    


def request_upload(dongle_id, paths, urls):
    headers = {
        "Authorization": f"JWT {authorization}",
        "Content-Type": "application/json"
    }

    # Collect all files data
    files_data = []
    for i, url in enumerate(urls):
        files_data.append({
            "fn": paths[i],
            "url": url['url'],
            "headers": {
                "x-ms-blob-type": "BlockBlob"
            }
        })

    # Send a single JSON-RPC request
    jsonrpc_request = {
        "jsonrpc": "2.0",
        "method": "uploadFilesToUrls",
        "params": {
            "files_data": files_data
        },
        "id": 0
    }
    try:
        response = requests.post(f'{base_url}/ws/{dongle_id}', headers=headers, data=json.dumps(jsonrpc_request), timeout=10)
    except requests.exceptions.Timeout:
        print(f"Timeout occurred while sending uploadFilesToUrls for {dongle_id}")
        return None
    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")
        return None
    if response.status_code == 200:
        pass
    else:
        print(f"Failed to send request: {response.status_code}")
        print("Response:", response.text)
        # Handle the error if needed, e.g., retry logic or logging

def is_downloaded(url):
    path = url.lstrip(base_url + "/connectdata/")
    path = path.split("?")[0]  # remove query params from URL
    path = os.path.join('downloads', path)
    return os.path.exists(path)



def download_file(url: str):
    path: str = url.lstrip(base_url + "/connectdata/qlog/")
    path = path.split("?")[0]  # remove query params from URL
    # https://connect-api.duckdns.org/connectdata/qlog/3b58edf884ab4eaf/2024-06-13--15-59-30/0/qlog.bz2
    path = os.path.join('downloads', path)
    if os.path.exists(path):
        print(f"File already exists: {path}")
        return
    response = requests.get(url)
    if response.status_code != 200:
        return
    
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'wb') as file:
        file.write(response.content)
    print(f"Downloaded and saved file: {path}")
