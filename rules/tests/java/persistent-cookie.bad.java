package demo;

import javax.servlet.http.Cookie;
import javax.servlet.http.HttpServletResponse;

// 탐지돼야 하는 경우: 쿠키 만료시간을 1년으로 길게 설정
public class RememberMe {
    public void remember(HttpServletResponse response) {
        Cookie loginCookie = new Cookie("rememberme", "YES");
        loginCookie.setMaxAge(60 * 60 * 24 * 365);
        response.addCookie(loginCookie);
    }
}
