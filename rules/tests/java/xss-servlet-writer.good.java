import java.io.*;
import javax.servlet.http.*;
import org.springframework.web.util.HtmlUtils;

class HelloServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String name = req.getParameter("name");
        resp.getWriter().println("<h1>Hello " + HtmlUtils.htmlEscape(name) + "</h1>");
    }
}
