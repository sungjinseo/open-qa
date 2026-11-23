package demo;

import com.auth0.jwt.JWT;
import com.auth0.jwt.interfaces.DecodedJWT;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import java.io.File;
import java.io.IOException;
import java.security.GeneralSecurityException;
import java.security.PublicKey;
import java.security.Signature;
import java.util.jar.JarFile;

// 탐지돼야 하는 경우 6건
public class PluginLoader {
    // 다운로드한 JAR 의 전자서명을 확인하지 않는다
    public JarFile open(String downloadedFilePath) throws IOException {
        return new JarFile(new File(downloadedFilePath));
    }

    // 서명 확인 생성자를 쓰지만 서명 주체(getCodeSigners)를 검증하지 않는다
    public JarFile openVerified(File f) throws IOException {
        return new JarFile(f, true);
    }

    // 서명 없는 JWT 를 받아들인다
    public Claims claims(String token) {
        return Jwts.parser().parseClaimsJwt(token).getBody();
    }

    // parse() 는 서명 없는 토큰도 그대로 받아들인다 (parseClaimsJws 를 써야 함)
    public Object any(String token, byte[] key) {
        return Jwts.parser().setSigningKey(key).parse(token).getBody();
    }

    // 서명을 검증하지 않고 토큰 내용을 꺼낸다
    public String subject(String token) {
        DecodedJWT jwt = JWT.decode(token);
        return jwt.getSubject();
    }

    // 서명 검증 결과를 버린다
    public byte[] load(byte[] data, byte[] sig, PublicKey key) throws GeneralSecurityException {
        Signature s = Signature.getInstance("SHA256withRSA");
        s.initVerify(key);
        s.update(data);
        s.verify(sig);
        return data;
    }
}
