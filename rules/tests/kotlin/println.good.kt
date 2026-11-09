import java.util.logging.Logger

private val log = Logger.getLogger("report")

fun report(s: String) {
    log.info("value=" + s)
}
