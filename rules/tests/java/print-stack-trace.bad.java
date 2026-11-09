class Job {
    void run() {
        try {
            Integer.parseInt("x");
        } catch (NumberFormatException e) {
            e.printStackTrace();
        }
    }
}
