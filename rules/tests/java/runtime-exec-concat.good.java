class Ping {
    void run(String host) throws java.io.IOException {
        if (!host.matches("[A-Za-z0-9.-]{1,253}")) throw new IllegalArgumentException("host");
        new ProcessBuilder("ping", "-c", "1", host).start();
    }
}
