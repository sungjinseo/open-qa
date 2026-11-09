fun parse(s: String): Int {
    return try {
        s.toInt()
    } catch (e: NumberFormatException) {
        System.err.println("invalid number: " + e.message)
        0
    }
}
