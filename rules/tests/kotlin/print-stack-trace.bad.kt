fun parse(s: String) {
    try {
        s.toInt()
    } catch (e: Exception) {
        e.printStackTrace()
    }
}
