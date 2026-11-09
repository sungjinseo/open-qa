import javax.servlet.http.*;

class Gate {
    boolean internal(HttpServletRequest req) {
        if (req.getRemoteAddr().startsWith("10.")) {
            return true;
        }
        return false;
    }
}
