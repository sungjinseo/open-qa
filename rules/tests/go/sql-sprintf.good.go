package store

import "database/sql"

func Find(db *sql.DB, id string) (*sql.Rows, error) {
	return db.Query("select * from users where id = $1", id)
}
