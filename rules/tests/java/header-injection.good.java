import java.net.URLEncoder;
import javax.servlet.http.*;

class LangServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String lang = req.getParameter("lang");
        resp.setHeader("X-Lang", URLEncoder.encode(lang, "UTF-8"));
    }
}
