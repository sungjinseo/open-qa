import java.net.URL;

class Client {
    URL api() throws Exception {
        return new URL("http://api.example.com/v1/users");
    }
}
