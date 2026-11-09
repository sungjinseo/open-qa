class Task {
    void run(Runnable r) {
        try {
            r.run();
        } catch (Throwable t) {
            System.err.println("failed");
        }
    }
}
