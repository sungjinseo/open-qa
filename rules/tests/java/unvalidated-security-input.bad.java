import javax.servlet.http.*;

class AdminServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        if ("true".equals(req.getParameter("isAdmin"))) {
            resp.getWriter().println("admin panel");
        }
    }
}
