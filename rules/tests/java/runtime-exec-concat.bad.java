class Ping {
    void run(String host) throws java.io.IOException {
        Runtime.getRuntime().exec("ping -c 1 " + host);
    }
}
