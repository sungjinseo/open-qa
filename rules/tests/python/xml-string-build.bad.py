import xml.etree.ElementTree as ET

def build(name):
    return ET.fromstring(f"<user><name>{name}</name></user>")
