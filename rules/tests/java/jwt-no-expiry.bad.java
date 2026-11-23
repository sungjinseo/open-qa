import io.jsonwebtoken.Jwts;
import java.security.Key;

class TokenService {
    String issue(String userId, String role, Key key) {
        return Jwts.builder().setSubject(userId).claim("role", role).signWith(key).compact();
    }
}
