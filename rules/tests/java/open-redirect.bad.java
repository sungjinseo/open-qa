import java.io.*;
import javax.servlet.http.*;

class LoginServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String next = req.getParameter("next");
        resp.sendRedirect(next);
    }
}
