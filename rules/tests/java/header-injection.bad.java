import javax.servlet.http.*;

class LangServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) {
        String lang = req.getParameter("lang");
        resp.setHeader("X-Lang", lang);
    }
}
