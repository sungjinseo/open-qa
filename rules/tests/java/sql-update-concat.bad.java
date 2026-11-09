import java.sql.*;

class UserDao {
    void rename(Statement s, String id, String name) throws SQLException {
        s.executeUpdate("UPDATE users SET name = '" + name + "' WHERE id = " + id);
    }
}
