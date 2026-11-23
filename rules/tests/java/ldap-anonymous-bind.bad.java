package demo;

import java.util.Hashtable;
import javax.naming.Context;
import javax.naming.NamingException;
import javax.naming.directory.InitialDirContext;
import javax.servlet.http.HttpServletRequest;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

// 탐지돼야 하는 경우 3건
@RestController
@RequestMapping("/admin")
public class BoardController {
    private final BoardDao boardDao;

    public BoardController(BoardDao boardDao) {
        this.boardDao = boardDao;
    }

    // 요청 파라미터의 action 값만 보고 권한 확인 없이 삭제한다
    public void handle(HttpServletRequest request) {
        String action = request.getParameter("action");
        String contentId = request.getParameter("contentId");
        if (action != null && action.equals("delete")) {
            boardDao.delete(contentId);
        }
    }

    // 관리자 경로인데 역할 검사가 없다
    @DeleteMapping("/boards/{id}")
    public void deleteBoard(@PathVariable long id) {
        boardDao.deleteBoard(id);
    }

    // 익명 바인딩으로 디렉터리 서버에 접속한다
    public InitialDirContext connect(String url) throws NamingException {
        Hashtable<String, String> env = new Hashtable<>();
        env.put(Context.PROVIDER_URL, url);
        env.put(Context.SECURITY_AUTHENTICATION, "none");
        return new InitialDirContext(env);
    }

    public interface BoardDao {
        void delete(String contentId);

        void deleteBoard(long id);
    }
}
