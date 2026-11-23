package demo;

import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.net.URL;
import java.text.SimpleDateFormat;
import java.util.Date;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

// 탐지돼야 하는 경우 2건 (진단가이드 4-3: 광범위한 Exception 처리, 반환값 미검사)
public class DateReader {
    private static final Logger LOG = LoggerFactory.getLogger(DateReader.class);

    public Date read(URL url) {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(url.openStream()))) {
            String line = reader.readLine();
            SimpleDateFormat format = new SimpleDateFormat("MM/dd/yy");
            return format.parse(line);
        } catch (Exception e) {
            // 예외를 세분화할 수 있는데도 광범위하게 처리한다
            LOG.error("ERROR-10: 날짜 읽기 실패");
            return null;
        }
    }

    public void cleanUp(File tmp) {
        // 삭제 실패를 알 수 없다
        tmp.delete();
    }
}
