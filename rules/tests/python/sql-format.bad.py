def delete_user(cursor, uid):
    cursor.execute("DELETE FROM users WHERE id = {}".format(uid))
