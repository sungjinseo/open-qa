import java.sql.*;
class UserDao {
    ResultSet find(Statement s, String id) throws SQLException {
        return s.executeQuery("select * from users where id=" + id);
    }
}
