import java.sql.Statement

fun find(s: Statement, id: String) = s.executeQuery("select * from users where id=" + id)
