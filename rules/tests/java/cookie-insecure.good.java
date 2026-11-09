import javax.servlet.http.*;

class SessionCookie {
    void issue(HttpServletResponse resp, String sid) {
        Cookie c = new Cookie("sid", sid);
        c.setHttpOnly(true);
        c.setSecure(true);
        resp.addCookie(c);
    }
}
