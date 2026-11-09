import java.sql.*;
class UserDao {
    int rename(Connection c, String name) throws SQLException {
        PreparedStatement ps = c.prepareStatement("update users set name=?");
        ps.setString(1, name);
        return ps.executeUpdate();
    }
}
