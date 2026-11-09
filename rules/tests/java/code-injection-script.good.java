import java.math.BigDecimal;

class Calc {
    BigDecimal run(String a, String b) {
        return new BigDecimal(a).add(new BigDecimal(b));
    }
}
