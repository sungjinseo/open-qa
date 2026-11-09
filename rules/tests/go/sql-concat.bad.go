package store

import "database/sql"

func Find(db *sql.DB, name string) (*sql.Rows, error) {
	return db.Query("select * from users where name = '" + name + "'")
}
