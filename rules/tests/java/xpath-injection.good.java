package demo;

import javax.servlet.http.HttpServletRequest;
import javax.xml.xpath.XPath;
import javax.xml.xpath.XPathExpressionException;
import javax.xml.xpath.XPathFactory;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.w3c.dom.Node;

// 탐지되면 안 되는 경우: 특수문자를 제거하거나(진단가이드 예), 질의문은 고정하고 값은 변수로 넘긴다
@RestController
public class AccountXPath {
    private final Node accounts;

    public AccountXPath(Node accounts) {
        this.accounts = accounts;
    }

    @GetMapping("/price")
    public String price(@RequestParam String name) throws XPathExpressionException {
        String safe = name.replaceAll("[()\\-'\\[\\]:,*/]", "");
        XPath xpath = XPathFactory.newInstance().newXPath();
        return xpath.evaluate("//food[name='" + safe + "']/price", accounts);
    }

    public String email(HttpServletRequest request) throws XPathExpressionException {
        String acctID = request.getParameter("acctID");
        XPath xpath = XPathFactory.newInstance().newXPath();
        xpath.setXPathVariableResolver(v -> acctID);
        return xpath.evaluate("/accounts/account[acctID=$id]/email/text()", accounts);
    }
}
