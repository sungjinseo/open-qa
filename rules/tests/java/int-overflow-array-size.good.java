class Grid {
    byte[] alloc(int width, int height) {
        return new byte[Math.multiplyExact(width, height)];
    }
}
