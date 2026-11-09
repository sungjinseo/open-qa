import javax.servlet.http.*;

class Gate {
    boolean internal(HttpServletRequest req) {
        if (req.getRemoteHost().endsWith(".corp.local")) {
            return true;
        }
        return false;
    }
}
