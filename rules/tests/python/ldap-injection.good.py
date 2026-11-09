from ldap3.utils.conv import escape_filter_chars

def find(conn, uid):
    return conn.search("dc=example,dc=com", "(uid=" + escape_filter_chars(uid) + ")")
