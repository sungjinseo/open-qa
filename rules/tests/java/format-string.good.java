import javax.servlet.http.*;

class MsgServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String name = req.getParameter("name");
        resp.getWriter().println(String.format("Hello %s", name));
    }
}
