import javax.servlet.http.*;

class DebugServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) {
        resp.setStatus(204);
    }
}
