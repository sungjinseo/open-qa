import org.springframework.web.bind.annotation.*;

@RestController
class UserController {
    private final UserRepository repo;

    UserController(UserRepository repo) { this.repo = repo; }

    @PutMapping("/users/{id}")
    User update(@PathVariable Long id, @RequestBody UserUpdateRequest req) {
        User user = repo.findById(id).orElseThrow();
        user.setName(req.getName());
        user.setEmail(req.getEmail());
        return repo.save(user);
    }
}

@RestController
class TagController {
    private final TagRepository repo;

    TagController(TagRepository repo) { this.repo = repo; }

    @PostMapping("/tags")
    Tag create(@Valid @RequestBody TagRequest req) {
        return repo.save(new Tag(req.getName()));
    }
}
