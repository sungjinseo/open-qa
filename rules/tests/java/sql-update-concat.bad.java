import java.sql.*;
class UserDao {
    int rename(Statement s, String name) throws SQLException {
        return s.executeUpdate("update users set name='" + name + "'");
    }
}
