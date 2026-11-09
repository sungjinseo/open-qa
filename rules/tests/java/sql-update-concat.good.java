import java.sql.*;

class UserDao {
    void rename(Connection c, String id, String name) throws SQLException {
        PreparedStatement ps = c.prepareStatement("UPDATE users SET name = ? WHERE id = ?");
        ps.setString(1, name);
        ps.setString(2, id);
        ps.executeUpdate();
    }
}
