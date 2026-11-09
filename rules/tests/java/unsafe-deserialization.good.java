import java.io.*;
class D {
    String read(InputStream in) throws Exception {
        return new BufferedReader(new InputStreamReader(in)).readLine();
    }
}
