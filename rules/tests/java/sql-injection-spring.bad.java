package demo;

import java.util.List;
import java.util.Map;
import javax.persistence.EntityManager;
import org.hibernate.Session;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

// 탐지돼야 하는 경우 4건: Spring 요청 파라미터를 질의문에 이어 붙임
@RestController
public class OwnerSearchController {
    private final JdbcTemplate jdbcTemplate;
    private final EntityManager em;
    private final Session session;

    public OwnerSearchController(JdbcTemplate jdbcTemplate, EntityManager em, Session session) {
        this.jdbcTemplate = jdbcTemplate;
        this.em = em;
        this.session = session;
    }

    // JdbcTemplate
    @GetMapping("/owners")
    public List<Map<String, Object>> byLastName(@RequestParam("lastName") String lastName) {
        return jdbcTemplate.queryForList("SELECT * FROM owners WHERE last_name LIKE '" + lastName + "%'");
    }

    // JPA 네이티브 질의 (진단가이드 예)
    @GetMapping("/items/{name}")
    public List<?> items(@PathVariable String name) {
        return em.createNativeQuery("SELECT * FROM item WHERE name = '" + name + "'").getResultList();
    }

    // Hibernate HQL (진단가이드 예)
    @GetMapping("/students")
    public List<?> students(@RequestParam String name) {
        return session.createQuery("from Student where studentName = '" + name + "'").list();
    }

    // StringBuilder 로 조립해도 같다
    @GetMapping("/pets")
    public List<Map<String, Object>> pets(@RequestParam String type) {
        StringBuilder sql = new StringBuilder("SELECT * FROM pets WHERE type = '");
        sql.append(type).append("'");
        return jdbcTemplate.queryForList(sql.toString());
    }
}
