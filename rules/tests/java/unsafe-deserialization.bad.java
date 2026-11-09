import java.io.*;

class Loader {
    Object load(InputStream in) throws Exception {
        return new ObjectInputStream(in).readObject();
    }
}
