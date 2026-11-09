import org.junit.jupiter.api.Test;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;

class Service {
    void process(String s) throws NumberFormatException {
        Integer.parseInt(s);
    }
    public static void main(String[] args) throws Exception {
        new Service().process("1");
    }
    @Test
    void worksInTests() throws Exception {
        process("1");
    }
    void configure(HttpSecurity http) throws Exception {
        http.csrf();
    }
}
