class DbConfig {
    private static final String DB_PASSWORD = System.getenv("DB_PASSWORD");
    private static final String PASSWORD_PARAM = "password";
    private static final String ENV_NAME = "DB_PASSWORD";
    private static final String SECRET_LABEL = "Secret";
    private static final String PW_KEY = "auth.password";
    private static final String PW_RULE = "password-required";
    private static final String PW_COL = "password_field";
    private static final String PW_PROMPT = "Password:";
    private static final String PW_MSG = "비밀번호를입력하세요";
}
