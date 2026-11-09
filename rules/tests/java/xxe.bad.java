import javax.xml.parsers.*;
import org.w3c.dom.Document;
import java.io.InputStream;

class XmlLoader {
    Document load(InputStream in) throws Exception {
        DocumentBuilderFactory f = DocumentBuilderFactory.newInstance();
        return f.newDocumentBuilder().parse(in);
    }
}
