package demo;

import com.auth0.jwt.JWT;
import com.auth0.jwt.algorithms.Algorithm;
import com.auth0.jwt.interfaces.DecodedJWT;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.security.CodeSigner;
import java.security.GeneralSecurityException;
import java.security.Key;
import java.security.PublicKey;
import java.security.Signature;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;

// 탐지되면 안 되는 경우: 서명을 확인하고 서명 주체·검증 결과를 사용한다
public class PluginLoader {
    public boolean isSigned(File f, String path) throws IOException {
        try (JarFile jf = new JarFile(f, true)) {
            JarEntry en = jf.getJarEntry(path);
            try (InputStream in = jf.getInputStream(en)) {
                in.readAllBytes();
            }
            CodeSigner[] signers = en.getCodeSigners();
            return signers != null && signers.length > 0;
        }
    }

    public Claims claims(String token, Key key) {
        return Jwts.parserBuilder().setSigningKey(key).build().parseClaimsJws(token).getBody();
    }

    public String subject(String token, Algorithm algorithm) {
        DecodedJWT jwt = JWT.require(algorithm).build().verify(token);
        return jwt.getSubject();
    }

    public byte[] load(byte[] data, byte[] sig, PublicKey key) throws GeneralSecurityException {
        Signature s = Signature.getInstance("SHA256withRSA");
        s.initVerify(key);
        s.update(data);
        if (!s.verify(sig)) {
            throw new SecurityException("서명이 일치하지 않습니다.");
        }
        return data;
    }
}
