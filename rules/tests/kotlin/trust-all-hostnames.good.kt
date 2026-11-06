import java.net.URL
import javax.net.ssl.HttpsURLConnection

fun open(u: String): HttpsURLConnection = URL(u).openConnection() as HttpsURLConnection
