import javax.servlet.http.*;

class NameServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String name = req.getParameter("name").trim();
        resp.getWriter().println(name.length());
    }
}
