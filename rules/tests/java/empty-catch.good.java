class Loader {
    void load() {
        try {
            Class.forName("x.Y");
        } catch (ClassNotFoundException e) {
            throw new IllegalStateException("missing plugin", e);
        }
    }
}
