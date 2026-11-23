package demo;

import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.MalformedURLException;
import java.net.URL;
import java.text.ParseException;
import java.text.SimpleDateFormat;
import java.util.Date;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

// 탐지되면 안 되는 경우 (진단가이드 4-3 안전한 코드: 발생 가능한 예외를 나눠 처리, 반환값 확인)
public class DateReader {
    private static final Logger LOG = LoggerFactory.getLogger(DateReader.class);

    public Date read(URL url) {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(url.openStream()))) {
            String line = reader.readLine();
            SimpleDateFormat format = new SimpleDateFormat("MM/dd/yy");
            return format.parse(line);
        } catch (MalformedURLException e) {
            LOG.error("ERROR-11: 주소 오류");
        } catch (IOException e) {
            LOG.error("ERROR-12: 읽기 실패");
        } catch (ParseException e) {
            LOG.error("ERROR-13: 날짜 형식 오류");
        }
        return new Date(0L);
    }

    public void cleanUp(File tmp) throws IOException {
        if (!tmp.delete()) {
            throw new IOException("ERROR-14: 임시 파일 삭제 실패");
        }
    }
}
