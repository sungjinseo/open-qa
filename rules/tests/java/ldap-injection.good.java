import javax.naming.directory.*;
import javax.naming.NamingException;

class LdapFinder {
    NamingEnumeration<SearchResult> find(DirContext ctx, String uid) throws NamingException {
        return ctx.search("ou=people,dc=example,dc=com", "(uid={0})", new Object[] {uid}, new SearchControls());
    }
}
