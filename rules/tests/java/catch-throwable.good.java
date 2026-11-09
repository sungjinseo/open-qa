import java.util.logging.Logger;

class Task {
    private static final Logger LOG = Logger.getLogger("task");
    void run(Runnable r) {
        try {
            r.run();
        } catch (IllegalStateException t) {
            System.err.println("failed");
        }
        try {
            r.run();
        } catch (Exception e) {
            throw new IllegalStateException("task failed", e);
        }
        try {
            r.run();
        } catch (Throwable t) {
            LOG.warn("task failed");
        }
    }
}
