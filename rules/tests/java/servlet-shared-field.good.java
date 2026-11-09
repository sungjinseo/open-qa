import javax.servlet.http.*;

class ProfileServlet extends HttpServlet {
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        String currentUser = req.getParameter("user");
        resp.getWriter().println(currentUser);
    }
}
