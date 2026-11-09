import xml.etree.ElementTree as ET

def build(name):
    user = ET.Element("user")
    ET.SubElement(user, "name").text = name
    return user
