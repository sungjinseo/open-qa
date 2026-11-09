fun run(r: Runnable) {
    try {
        r.run()
    } catch (e: IllegalStateException) {
        System.err.println(e)
    }
}
