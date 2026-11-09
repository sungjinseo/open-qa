import javax.servlet.http.*;

class ProfileServlet extends HttpServlet {
    private String currentUser;
    protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws Exception {
        currentUser = req.getParameter("user");
        resp.getWriter().println(currentUser);
    }
}
