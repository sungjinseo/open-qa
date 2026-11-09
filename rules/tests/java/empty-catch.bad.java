class Loader {
    void load() {
        try {
            Class.forName("x.Y");
        } catch (ClassNotFoundException e) {
        }
    }
}
