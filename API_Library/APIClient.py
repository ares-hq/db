import os
import requests
from requests.adapters import HTTPAdapter
from dotenv import load_dotenv
from urllib3.util.retry import Retry

class APIClient:
    """
    A simple and flexible API client for making requests.
    """
    def __init__(self, base_url):
        """
        Initialize the API client.
        :param base_url: (str) The base URL of the API.
        """
        load_dotenv()
        self.base_url = base_url.rstrip('/')
        self.username = os.getenv('FIRST_USERNAME')
        self.password = os.getenv('FIRST_PASS')

        if not self.username or not self.password:
            raise EnvironmentError("Environment variables API_USERNAME and API_PASSWORD are required.")

        self.session = requests.Session()
        self.session.auth = (self.username, self.password)
        
        adapter = HTTPAdapter(
            pool_connections=500,
            pool_maxsize=500,
            max_retries=Retry(total=3, backoff_factor=0.3),
            pool_block=True
        )
        self.session.mount("https://", adapter)
        self.session.mount("http://", adapter)

    def build_url(self, apiParams):
        """
        Build a URL using the base URL, path segments, and optional query parameters.
        :param path_segments: (list) List of path segments.
        :param params: (dict, optional) Query parameters for the URL.
        :return: (str) Constructed URL.
        """
        path = "/".join(str(segment) for segment in apiParams.path_segments if segment)

        url = f"{self.base_url}/{path}"

        if apiParams.query_params:
            query_string = "&".join(f"{key}={value}" for key, value in apiParams.query_params.items())
            url = f"{url}?{query_string}"

        return url
    
    def api_request(self, api_params, params=None, headers=None):
        """
        Make a GET request to the API.
        :param path_segments: (list) Path segments for the URL.
        :param params: (dict) Query parameters for the request.
        :param headers: (dict) Additional headers for the request.
        :return: (dict) JSON response from the API.
        """
        url = self.build_url(api_params)
        response = self.session.get(url, params=params, headers=headers)

        if not response.ok:
            response.raise_for_status()

        return response.json()
    
    def post_request(self, path_segments, data=None, headers=None):
        """
        Make a POST request to the API.
        :param path_segments: (list) Path segments for the URL.
        :param data: (dict) Data to send in the POST request.
        :param headers: (dict) Additional headers for the request.
        :return: (dict) JSON response from the API.
        """
        url = self.build_url(path_segments)
        response = self.session.post(url, json=data, headers=headers)

        if not response.ok:
            response.raise_for_status()

        return response.json()

    def get_last_update(self):
        """
        Get the last update time from the API.
        :return: (str) Last update timestamp.
        """
        response = self.api_request(['last_update'])
        return response.get('lastUpdate')