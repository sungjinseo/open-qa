import java.util.concurrent.*;
class W {
    void later(ScheduledExecutorService ses, Runnable r) {
        ses.schedule(r, 1, TimeUnit.SECONDS);
    }
}
