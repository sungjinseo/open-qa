import com.auth0.jwt.JWT;

class Auth {
    String user(String token) {
        return JWT.decode(token).getSubject();
    }
}
