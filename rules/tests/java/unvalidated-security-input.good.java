import javax.servlet.http.*;

class AdminServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        if (req.isUserInRole("ADMIN")) {
            resp.getWriter().println("admin panel");
        }
    }
}
