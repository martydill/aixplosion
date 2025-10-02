import requests
import urllib.request
import urllib.parse
import json

def make_request_with_requests():
    """
    Example using the requests library (recommended approach)
    You need to install it first: pip install requests
    """
    try:
        # GET request
        response = requests.get('https://httpbin.org/get')
        print(f"Status Code: {response.status_code}")
        print(f"Response: {response.json()}")
        
        # POST request with JSON data
        data = {'key': 'value', 'name': 'John'}
        headers = {'Content-Type': 'application/json'}
        
        response = requests.post(
            'https://httpbin.org/post',
            data=json.dumps(data),
            headers=headers
        )
        print(f"\nPOST Status Code: {response.status_code}")
        print(f"POST Response: {response.json()}")
        
        # GET request with parameters
        params = {'param1': 'value1', 'param2': 'value2'}
        response = requests.get('https://httpbin.org/get', params=params)
        print(f"\nGET with params Status Code: {response.status_code}")
        print(f"GET with params Response: {response.json()}")
        
    except requests.exceptions.RequestException as e:
        print(f"Error occurred: {e}")

def make_request_with_urllib():
    """
    Example using urllib (built-in Python library)
    """
    try:
        # GET request
        url = 'https://httpbin.org/get'
        with urllib.request.urlopen(url) as response:
            data = json.loads(response.read().decode())
            print(f"urllib GET Response: {data}")
        
        # POST request
        url = 'https://httpbin.org/post'
        post_data = {'key': 'value', 'name': 'John'}
        data_bytes = urllib.parse.urlencode(post_data).encode('utf-8')
        
        req = urllib.request.Request(
            url,
            data=data_bytes,
            headers={'Content-Type': 'application/x-www-form-urlencoded'}
        )
        
        with urllib.request.urlopen(req) as response:
            result = json.loads(response.read().decode())
            print(f"urllib POST Response: {result}")
            
    except Exception as e:
        print(f"Error occurred: {e}")

def make_request_with_custom_headers():
    """
    Example with custom headers and authentication
    """
    try:
        headers = {
            'User-Agent': 'MyPythonApp/1.0',
            'Accept': 'application/json',
            'Authorization': 'Bearer your_token_here'
        }
        
        response = requests.get(
            'https://httpbin.org/headers',
            headers=headers
        )
        print(f"Custom Headers Response: {response.json()}")
        
    except requests.exceptions.RequestException as e:
        print(f"Error occurred: {e}")

if __name__ == "__main__":
    print("=== Making HTTP Requests with requests library ===")
    make_request_with_requests()
    
    print("\n=== Making HTTP Requests with urllib ===")
    make_request_with_urllib()
    
    print("\n=== Making HTTP Requests with custom headers ===")
    make_request_with_custom_headers()