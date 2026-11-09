import java.io.*;
import java.nio.file.*;

class Store {
    void write(Path p) throws IOException {
        try {
            Files.createFile(p);
        } catch (FileAlreadyExistsException e) {
            return;
        }
    }
}
