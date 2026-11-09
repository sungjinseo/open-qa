def find(conn, uid):
    return conn.search("dc=example,dc=com", f"(uid={uid})")
