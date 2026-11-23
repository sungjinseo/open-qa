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

// 탐지되면 안 되는 경우: 바인딩 변수·이름 있는 파라미터 사용, 숫자형 파라미터
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

    @GetMapping("/owners")
    public List<Map<String, Object>> byLastName(@RequestParam("lastName") String lastName) {
        return jdbcTemplate.queryForList("SELECT * FROM owners WHERE last_name LIKE ?", lastName + "%");
    }

    @GetMapping("/items/{name}")
    public List<?> items(@PathVariable String name) {
        return em.createNativeQuery("SELECT * FROM item WHERE name = ?1").setParameter(1, name).getResultList();
    }

    @GetMapping("/students")
    public List<?> students(@RequestParam String name) {
        return session.createQuery("from Student where studentName = :name").setParameter("name", name).list();
    }

    // 숫자형 파라미터는 문자열을 끼워 넣을 수 없다
    @GetMapping("/pets/{id}")
    public List<Map<String, Object>> pet(@PathVariable int id) {
        return jdbcTemplate.queryForList("SELECT * FROM pets WHERE id = " + id);
    }
}
