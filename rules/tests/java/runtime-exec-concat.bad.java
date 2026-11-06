class Shell {
    void ping(String host) throws Exception {
        Runtime.getRuntime().exec("ping -c 1 " + host);
    }
}
