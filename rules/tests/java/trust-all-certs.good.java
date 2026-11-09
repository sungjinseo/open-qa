import java.security.cert.*;
import javax.net.ssl.X509TrustManager;

class Pinned implements X509TrustManager {
    private final X509TrustManager delegate;
    Pinned(X509TrustManager d) { this.delegate = d; }
    public void checkClientTrusted(X509Certificate[] chain, String type) throws CertificateException {
        delegate.checkClientTrusted(chain, type);
    }
    public void checkServerTrusted(X509Certificate[] chain, String type) throws CertificateException {
        delegate.checkServerTrusted(chain, type);
    }
    public X509Certificate[] getAcceptedIssuers() { return delegate.getAcceptedIssuers(); }
}
