fun connect() {
    val dbPassword = System.getenv("DB_PASSWORD") ?: error("DB_PASSWORD is not set")
    val user = "admin"
    open(user, dbPassword)
}
fun open(u: String, p: String) {}
