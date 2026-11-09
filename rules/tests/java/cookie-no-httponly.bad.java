import javax.servlet.http.*;

class PrefCookie {
    void issue(HttpServletResponse resp, String v) {
        Cookie c = new Cookie("pref", v);
        resp.addCookie(c);
    }
}
