import javax.net.ssl.HttpsURLConnection

fun init() {
    HttpsURLConnection.setDefaultHostnameVerifier { _, _ -> true }
}
