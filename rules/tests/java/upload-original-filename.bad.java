import java.io.File;
import java.io.IOException;
import org.springframework.web.multipart.MultipartFile;

class Uploader {
    void save(MultipartFile file) throws IOException {
        File dest = new File("/data/uploads", file.getOriginalFilename());
        file.transferTo(dest);
    }
}
