import java.io.*;
import java.net.URL;
import javax.servlet.http.*;

class ProxyServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String key = req.getParameter("site");
        if (!"docs".equals(key)) { resp.sendError(400); return; }
        resp.getOutputStream().write(new URL("https://docs.example.com/").openStream().readAllBytes());
    }
}
