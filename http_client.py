#!/usr/bin/env python3
"""
HTTP Requests Examples in Python
This file demonstrates various ways to make HTTP requests using the requests library.
"""

import requests
import json
from typing import Dict, Any, Optional

# First, install the requests library if you haven't already:
# pip install requests


def basic_get_request(url: str) -> Optional[requests.Response]:
    """
    Make a basic GET request to a URL.
    
    Args:
        url: The URL to make the request to
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.get(url)
        response.raise_for_status()  # Raise an exception for bad status codes (4xx or 5xx)
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making GET request: {e}")
        return None


def get_with_parameters(base_url: str, params: Dict[str, Any]) -> Optional[requests.Response]:
    """
    Make a GET request with query parameters.
    
    Args:
        base_url: The base URL
        params: Dictionary of query parameters
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.get(base_url, params=params)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making GET request with params: {e}")
        return None


def post_json_data(url: str, data: Dict[str, Any], headers: Optional[Dict[str, str]] = None) -> Optional[requests.Response]:
    """
    Make a POST request with JSON data.
    
    Args:
        url: The URL to post to
        data: Dictionary of data to send as JSON
        headers: Optional additional headers
        
    Returns:
        Response object or None if request fails
    """
    try:
        # Default headers for JSON
        default_headers = {'Content-Type': 'application/json'}
        if headers:
            default_headers.update(headers)
            
        response = requests.post(url, json=data, headers=default_headers)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making POST request with JSON: {e}")
        return None


def post_form_data(url: str, data: Dict[str, Any], headers: Optional[Dict[str, str]] = None) -> Optional[requests.Response]:
    """
    Make a POST request with form data.
    
    Args:
        url: The URL to post to
        data: Dictionary of form data
        headers: Optional additional headers
        
    Returns:
        Response object or None if request fails
    """
    try:
        # Default headers for form data
        default_headers = {'Content-Type': 'application/x-www-form-urlencoded'}
        if headers:
            default_headers.update(headers)
            
        response = requests.post(url, data=data, headers=default_headers)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making POST request with form data: {e}")
        return None


def put_request(url: str, data: Dict[str, Any]) -> Optional[requests.Response]:
    """
    Make a PUT request to update data.
    
    Args:
        url: The URL to send PUT request to
        data: Dictionary of data to send
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.put(url, json=data)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making PUT request: {e}")
        return None


def delete_request(url: str) -> Optional[requests.Response]:
    """
    Make a DELETE request.
    
    Args:
        url: The URL to send DELETE request to
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.delete(url)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making DELETE request: {e}")
        return None


def request_with_custom_headers(url: str, headers: Dict[str, str]) -> Optional[requests.Response]:
    """
    Make a request with custom headers.
    
    Args:
        url: The URL to make request to
        headers: Dictionary of custom headers
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.get(url, headers=headers)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making request with custom headers: {e}")
        return None


def request_with_timeout(url: str, timeout: int = 10) -> Optional[requests.Response]:
    """
    Make a request with a timeout.
    
    Args:
        url: The URL to make request to
        timeout: Timeout in seconds
        
    Returns:
        Response object or None if request fails
    """
    try:
        response = requests.get(url, timeout=timeout)
        response.raise_for_status()
        return response
    except requests.exceptions.RequestException as e:
        print(f"Error making request with timeout: {e}")
        return None


def download_file(url: str, save_path: str) -> bool:
    """
    Download a file from a URL.
    
    Args:
        url: URL of the file to download
        save_path: Local path to save the file
        
    Returns:
        True if successful, False otherwise
    """
    try:
        response = requests.get(url, stream=True)
        response.raise_for_status()
        
        with open(save_path, 'wb') as file:
            for chunk in response.iter_content(chunk_size=8192):
                file.write(chunk)
        
        print(f"File downloaded successfully to {save_path}")
        return True
    except requests.exceptions.RequestException as e:
        print(f"Error downloading file: {e}")
        return False
    except IOError as e:
        print(f"Error saving file: {e}")
        return False


def handle_response(response: requests.Response) -> None:
    """
    Handle and display response information.
    
    Args:
        response: The response object to handle
    """
    if response is None:
        print("No response to handle")
        return
    
    print(f"Status Code: {response.status_code}")
    print(f"Headers: {dict(response.headers)}")
    
    # Try to parse JSON response
    try:
        json_data = response.json()
        print(f"JSON Response: {json.dumps(json_data, indent=2)}")
    except ValueError:
        # If not JSON, print text response
        print(f"Text Response: {response.text}")


# Example usage and demonstration
if __name__ == "__main__":
    # Example 1: Basic GET request
    print("=== Example 1: Basic GET Request ===")
    response = basic_get_request("https://httpbin.org/get")
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 2: GET request with parameters
    print("=== Example 2: GET Request with Parameters ===")
    params = {"key1": "value1", "key2": "value2"}
    response = get_with_parameters("https://httpbin.org/get", params)
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 3: POST request with JSON data
    print("=== Example 3: POST Request with JSON ===")
    json_data = {"name": "John Doe", "email": "john@example.com", "age": 30}
    response = post_json_data("https://httpbin.org/post", json_data)
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 4: POST request with form data
    print("=== Example 4: POST Request with Form Data ===")
    form_data = {"username": "user123", "password": "pass123"}
    response = post_form_data("https://httpbin.org/post", form_data)
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 5: Request with custom headers
    print("=== Example 5: Request with Custom Headers ===")
    headers = {"User-Agent": "MyApp/1.0", "Accept": "application/json"}
    response = request_with_custom_headers("https://httpbin.org/headers", headers)
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 6: PUT request
    print("=== Example 6: PUT Request ===")
    put_data = {"id": 1, "name": "Updated Name", "status": "active"}
    response = put_request("https://httpbin.org/put", put_data)
    if response:
        handle_response(response)
    
    print("\n" + "="*50 + "\n")
    
    # Example 7: DELETE request
    print("=== Example 7: DELETE Request ===")
    response = delete_request("https://httpbin.org/delete")
    if response:
        handle_response(response)