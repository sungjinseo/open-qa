class E {
    int f() {
        try {
            return Integer.parseInt("x");
        } catch (NumberFormatException e) {
            System.err.println("invalid number: " + e.getMessage());
            return 0;
        }
    }
}
