def find(cur, user_id):
    cur.execute(f"select * from users where id = {user_id}")
    return cur.fetchall()
