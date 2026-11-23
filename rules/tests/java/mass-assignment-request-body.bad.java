import org.springframework.web.bind.annotation.*;

@RestController
class UserController {
    private final UserRepository repo;

    UserController(UserRepository repo) { this.repo = repo; }

    @PutMapping("/users/{id}")
    User update(@PathVariable Long id, @RequestBody User user) {
        user.setId(id);
        return repo.save(user);
    }
}
