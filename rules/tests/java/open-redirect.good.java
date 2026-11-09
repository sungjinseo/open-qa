import java.io.*;
import java.util.Set;
import javax.servlet.http.*;

class LoginServlet extends HttpServlet {
    private static final Set<String> ALLOWED = Set.of("/home", "/profile");
    static String safeRedirectTarget(String next) {
        return ALLOWED.contains(next) ? next : "/home";
    }
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String next = req.getParameter("next");
        resp.sendRedirect(safeRedirectTarget(next));
    }
}
