import com.auth0.jwt.JWT;
import com.auth0.jwt.algorithms.Algorithm;

class Auth {
    String user(String token, byte[] key) {
        return JWT.require(Algorithm.HMAC256(key)).build().verify(token).getSubject();
    }
}
