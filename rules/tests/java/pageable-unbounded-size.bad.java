import org.springframework.data.domain.*;
import org.springframework.web.bind.annotation.*;

@RestController
class OrderController {
    private final OrderRepository repo;

    OrderController(OrderRepository repo) { this.repo = repo; }

    @GetMapping("/orders")
    Page<Order> list(@RequestParam(defaultValue = "0") int page, @RequestParam(defaultValue = "20") int size) {
        return repo.findAll(PageRequest.of(page, size));
    }
}
