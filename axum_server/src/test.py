import requests
import json

def get_json_response(url):
    """
    Makes an HTTP GET request to the given URL and prints the JSON response.
    
    Args:
    url (str): The URL to which the request is sent.
    
    Returns:
    None
    """
    try:
        # Send the request
        response = requests.get(url)
        
        # Check if the request was successful
        response.raise_for_status()
        
        # Get JSON response
        json_data = response.json()
        
        # Print the JSON response
        print(json.dumps(json_data, indent=4))
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")  # HTTP error
    except Exception as err:
        print(f"An error occurred: {err}")  # Other errors

if __name__ == "__main__":
    # URL of the endpoint
    url = "http://127.0.0.1:6734/v1/route/164080f7933651c4|2024-03-02--19-02-46/files"
    
    # Call the function with the URL
    get_json_response(url)