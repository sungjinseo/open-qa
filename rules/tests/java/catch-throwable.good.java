class Task {
    void run(Runnable r) {
        try {
            r.run();
        } catch (IllegalStateException t) {
            System.err.println("failed");
        }
    }
}
