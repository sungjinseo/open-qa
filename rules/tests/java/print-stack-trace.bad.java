class L {
    void f() {
        try {
            Integer.parseInt("x");
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
