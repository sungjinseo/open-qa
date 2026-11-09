package store

import (
	"database/sql"
	"fmt"
)

func Find(db *sql.DB, id string) (*sql.Rows, error) {
	return db.Query(fmt.Sprintf("select * from users where id = '%s'", id))
}
