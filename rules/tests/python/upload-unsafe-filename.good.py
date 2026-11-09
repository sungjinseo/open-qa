import os
import uuid
from flask import request

def upload():
    f = request.files["file"]
    f.save(os.path.join("/data/uploads", uuid.uuid4().hex + ".dat"))
