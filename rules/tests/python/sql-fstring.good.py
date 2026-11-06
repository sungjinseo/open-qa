def find(cur, user_id):
    cur.execute("select * from users where id = %s", (user_id,))
    return cur.fetchall()
