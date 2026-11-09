import java.sql.*;
class UserDao {
    ResultSet find(Connection c, String id) throws SQLException {
        PreparedStatement ps = c.prepareStatement("select * from users where id=?");
        ps.setString(1, id);
        return ps.executeQuery();
    }
}
