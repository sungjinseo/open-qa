import javax.servlet.http.*;

class ErrServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws java.io.IOException {
        try {
            Integer.parseInt(req.getParameter("n"));
        } catch (Exception e) {
            log("parse failed", e);
            resp.sendError(400, "invalid request");
        }
    }
}
