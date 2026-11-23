import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

@RestController
class OrderController {
    private final OrderRepository repo;

    OrderController(OrderRepository repo) { this.repo = repo; }

    @GetMapping("/orders/{id}")
    Order get(@PathVariable Long id, @AuthenticationPrincipal AppUser me) {
        return repo.findByIdAndOwnerId(id, me.getId()).orElseThrow();
    }
}
