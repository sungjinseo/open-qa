import java.io.*;
class D {
    Object read(InputStream in) throws Exception {
        return new ObjectInputStream(in).readObject();
    }
}
