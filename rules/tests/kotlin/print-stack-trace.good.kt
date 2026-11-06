import java.util.logging.Level
import java.util.logging.Logger

private val log = Logger.getLogger("parse")

fun parse(s: String) {
    try {
        s.toInt()
    } catch (e: Exception) {
        log.log(Level.WARNING, "parse failed", e)
    }
}
