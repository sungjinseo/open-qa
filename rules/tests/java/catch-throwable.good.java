class E {
    void f(Runnable r) {
        try {
            r.run();
        } catch (IllegalStateException e) {
            System.err.println(e);
        }
    }
}
