import javax.net.ssl.*;
import java.net.URL;
class T {
    HttpsURLConnection open(String u) throws Exception {
        return (HttpsURLConnection) new URL(u).openConnection();
    }
}
