class Worker {
    void halt(Thread t) {
        t.interrupt();
    }
}
