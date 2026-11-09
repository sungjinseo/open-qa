import javax.servlet.http.*;

class NameServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String raw = req.getParameter("name");
        String name = raw == null ? "" : raw.trim();
        resp.getWriter().println(name.length());
    }
}
