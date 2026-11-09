import java.util.logging.*;
class L {
    private static final Logger LOG = Logger.getLogger("L");
    void f() {
        try {
            Integer.parseInt("x");
        } catch (Exception e) {
            LOG.log(Level.WARNING, "parse failed", e);
        }
    }
}
