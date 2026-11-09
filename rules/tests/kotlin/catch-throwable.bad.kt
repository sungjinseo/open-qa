fun run(r: Runnable) {
    try {
        r.run()
    } catch (t: Throwable) {
        System.err.println(t)
    }
}
