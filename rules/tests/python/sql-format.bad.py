def find(cur, user_id):
    cur.execute("select * from users where id = {}".format(user_id))
    return cur.fetchall()
