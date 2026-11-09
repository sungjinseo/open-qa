import os

DB_PASSWORD = os.environ["DB_PASSWORD"]
token_type = "Bearer"
PASSWORD_FIELD = "password"
ENV_NAME = "DB_PASSWORD"
secret_label = "Secret"
password_key = "auth.password"
password_rule = "password-required"
password_col = "password_field"
password_prompt = "Password:"
password_msg = "비밀번호를입력하세요"
