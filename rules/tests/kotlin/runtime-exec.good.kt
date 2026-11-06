fun ping(host: String) {
    require(host.matches(Regex("[A-Za-z0-9.-]+")))
    ProcessBuilder("ping", "-c", "1", host).start()
}
