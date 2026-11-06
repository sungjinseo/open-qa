class E {
    void f(Runnable r) {
        try {
            r.run();
        } catch (Throwable t) {
            System.err.println(t);
        }
    }
}
