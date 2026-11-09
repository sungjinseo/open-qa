import java.io.*;
import javax.servlet.http.*;

class DownloadServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws IOException {
        String name = req.getParameter("file");
        File f = new File("/data/uploads/" + name);
        resp.getOutputStream().write(java.nio.file.Files.readAllBytes(f.toPath()));
    }
}
