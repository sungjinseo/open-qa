import java.util.logging.*;
class L {
    private static final Logger LOG = Logger.getLogger("L");
    void f(String s) {
        LOG.info("value=" + s);
    }
}
