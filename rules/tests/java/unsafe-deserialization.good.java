import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.InputStream;

class Loader {
    Object load(InputStream in) throws Exception {
        return new ObjectMapper().readValue(in, java.util.Map.class);
    }
}
