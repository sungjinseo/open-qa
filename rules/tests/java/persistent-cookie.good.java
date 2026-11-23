package demo;

import javax.servlet.http.Cookie;
import javax.servlet.http.HttpServletResponse;

// 탐지되면 안 되는 경우: 만료시간을 기능에 맞춰 최소(1일)로 설정
public class RememberMe {
    public void remember(HttpServletResponse response) {
        Cookie loginCookie = new Cookie("rememberme", "YES");
        loginCookie.setMaxAge(60 * 60 * 24);
        response.addCookie(loginCookie);
    }
}
