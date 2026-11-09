import java.io.*;

class Store {
    void write(File f) throws IOException {
        if (!f.exists()) {
            try (FileOutputStream out = new FileOutputStream(f)) {
                out.write(1);
            }
        }
    }
}
