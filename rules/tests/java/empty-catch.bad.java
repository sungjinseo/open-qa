class E {
    void f() {
        try {
            Integer.parseInt("x");
        } catch (NumberFormatException e) {
        }
    }
}
