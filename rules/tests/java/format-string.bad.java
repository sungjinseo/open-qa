import javax.servlet.http.*;

class MsgServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String tpl = req.getParameter("tpl");
        resp.getWriter().println(String.format(tpl, "user"));
    }
}
