fun ping(host: String) {
    Runtime.getRuntime().exec("ping -c 1 " + host)
}
