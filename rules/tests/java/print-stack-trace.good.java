import java.util.logging.*;

class Job {
    private static final Logger LOG = Logger.getLogger(Job.class.getName());
    void run() {
        try {
            Integer.parseInt("x");
        } catch (NumberFormatException e) {
            LOG.log(Level.WARNING, "parse failed", e);
        }
    }
}
