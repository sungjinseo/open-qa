import java.sql.*;

class UserDao {
    ResultSet find(Connection c, String name) throws SQLException {
        Statement s = c.createStatement();
        return s.executeQuery("SELECT * FROM users WHERE name = '" + name + "'");
    }
}
