import java.io.File;
import java.io.IOException;
import java.util.UUID;
import org.springframework.web.multipart.MultipartFile;

class Uploader {
    void save(MultipartFile file) throws IOException {
        File dest = new File("/data/uploads", UUID.randomUUID() + ".dat");
        file.transferTo(dest);
    }
}
