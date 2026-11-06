class Conf {
    void init() {
        String dbPassword = System.getenv("DB_PASSWORD");
        String user = "admin";
        connect(user, dbPassword);
    }
    void connect(String u, String p) {}
}
