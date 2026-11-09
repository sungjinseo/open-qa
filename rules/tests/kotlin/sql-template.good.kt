import java.sql.Connection

fun find(c: Connection, id: String) =
    c.prepareStatement("select * from users where id=?").use {
        it.setString(1, id)
        it.executeQuery()
    }
