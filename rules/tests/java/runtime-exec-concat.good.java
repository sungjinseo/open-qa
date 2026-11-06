class Shell {
    void ping(String host) throws Exception {
        if (!host.matches("[A-Za-z0-9.-]+")) throw new IllegalArgumentException(host);
        new ProcessBuilder("ping", "-c", "1", host).start();
    }
}
