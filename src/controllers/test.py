# test_get_upload_url.py

import requests

def main():
   # Define the server address and endpoint path
   server_address = "http://localhost:3111"  # Change as needed
   dongle_id = "ccfab3437bea5257"
   endpoint_path = f"/v1.4/{dongle_id}/upload_url/"

   # Define the query parameters
   params = {
      "path": "2019-06-06--11-30-31--9/fcamera.hevc",
      "expiry_days": 1
   }

   # Make the GET request to the endpoint
   response = requests.get(f"{server_address}{endpoint_path}", params=params)

   # Check that the response was successful
   assert response.status_code == 200, f"Unexpected status code: {response.status_code}"

   # Parse the response as JSON
   response_data = response.json()

   # Ensure it has a URL key
   assert "url" in response_data, "Response does not contain 'url'."

   # Optionally, you can check for specific values or structures
   assert response_data["url"].startswith(server_address), f"Unexpected URL format: {response_data['url']}"

   print("Test passed successfully!")

# Run the test when the script is executed directly
if __name__ == "__main__":
   main()
