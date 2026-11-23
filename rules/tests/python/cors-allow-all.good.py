from flask import Flask
from flask_cors import CORS

app = Flask(__name__)
CORS(app, origins=["https://app.example.com"], supports_credentials=True)
