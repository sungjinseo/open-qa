import io.jsonwebtoken.Jwts;
import java.security.Key;
import java.util.Date;

class TokenService {
    String issue(String userId, String role, Key key) {
        Date exp = new Date(System.currentTimeMillis() + 15 * 60 * 1000);
        return Jwts.builder().setSubject(userId).claim("role", role).setExpiration(exp).signWith(key).compact();
    }
}
