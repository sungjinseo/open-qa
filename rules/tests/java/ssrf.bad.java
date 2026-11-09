import java.io.*;
import java.net.URL;
import javax.servlet.http.*;

class ProxyServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String target = req.getParameter("url");
        resp.getOutputStream().write(new URL(target).openStream().readAllBytes());
    }
}
