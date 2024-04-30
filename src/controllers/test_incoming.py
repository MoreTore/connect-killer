import requests

def upload_file(file_path, server_url, headers):
    """Uploads a file to the specified server URL with headers."""

    # Open the file in binary mode
    with open(file_path, "rb") as f:
        # Create the PUT request with file data as the payload
        response = requests.post(server_url, data=f, headers=headers)

    # Check the server's response
    if response.status_code in [200, 201]:
        print("File uploaded successfully!")
    else:
        print(f"File upload failed with status code {response.status_code}")
        print(f"Server response: {response.text}")

# Replace with your actual file path and server URL
file_path = "/root/connect/406f02914de1a867_2024-02-05--16-22-28--10--qlog.bz2"
server_url = "http://localhost:3111/connectincoming/ccfab3437dbea5257/2023-03-02--19-02-46/0/rlog.bz2"  # Replace with the actual server URL

# Define headers for the request
headers = {
    "Content-Type": "application/octet-stream",   # Or another MIME type if necessary
    # Add other headers as needed
}

# Call the function to upload the file
upload_file(file_path, server_url, headers)