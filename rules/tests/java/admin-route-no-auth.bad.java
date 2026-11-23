import org.springframework.web.bind.annotation.*;

@RestController
class AdminController {
    private final UserService service;

    AdminController(UserService service) { this.service = service; }

    @PostMapping("/admin/users/{id}/role")
    void changeRole(@PathVariable Long id, @RequestParam String role) {
        service.changeRole(id, role);
    }
}
