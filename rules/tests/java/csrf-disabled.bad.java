import org.springframework.security.config.annotation.web.builders.HttpSecurity;

class SecurityConfig {
    void configure(HttpSecurity http) throws Exception {
        http.csrf().disable().authorizeRequests().anyRequest().authenticated();
    }
}
