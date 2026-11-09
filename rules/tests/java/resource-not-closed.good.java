import java.io.*;

class Reader2 {
    int first(String path) throws IOException {
        try (FileInputStream in = new FileInputStream(path)) {
            return in.read();
        }
    }
}
