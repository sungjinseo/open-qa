package demo;

import java.util.Hashtable;
import javax.naming.Context;
import javax.naming.NamingException;
import javax.naming.directory.InitialDirContext;
import javax.servlet.http.HttpServletRequest;
import javax.servlet.http.HttpSession;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

// 탐지되면 안 되는 경우: 역할·ACL 을 확인하고, 디렉터리 서버에는 자격증명으로 접속한다
@RestController
@RequestMapping("/admin")
@PreAuthorize("hasRole('ADMIN')")
public class BoardController {
    private final BoardDao boardDao;
    private final AccessControl acl;

    public BoardController(BoardDao boardDao, AccessControl acl) {
        this.boardDao = boardDao;
        this.acl = acl;
    }

    public void handle(HttpServletRequest request, HttpSession session) {
        String action = request.getParameter("action");
        String contentId = request.getParameter("contentId");
        Object user = session.getAttribute("user");
        if ("delete".equals(action) && acl.checkAccessControlList(user, action)) {
            boardDao.delete(contentId);
        }
    }

    @DeleteMapping("/boards/{id}")
    public void deleteBoard(@PathVariable long id) {
        boardDao.deleteBoard(id);
    }

    public InitialDirContext connect(String url, String principal, String credential) throws NamingException {
        Hashtable<String, String> env = new Hashtable<>();
        env.put(Context.PROVIDER_URL, url);
        env.put(Context.SECURITY_AUTHENTICATION, "simple");
        env.put(Context.SECURITY_PRINCIPAL, principal);
        env.put(Context.SECURITY_CREDENTIALS, credential);
        return new InitialDirContext(env);
    }

    public interface BoardDao {
        void delete(String contentId);

        void deleteBoard(long id);
    }

    public interface AccessControl {
        boolean checkAccessControlList(Object user, String action);
    }
}
