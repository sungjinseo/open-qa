import os

db_password = os.environ["DB_PASSWORD"]
db_user = "admin"

def connect():
    return (db_user, db_password)
