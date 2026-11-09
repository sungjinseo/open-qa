import java.util.Random;

class TokenMaker {
    String token() {
        return Long.toHexString(new Random().nextLong());
    }
}
