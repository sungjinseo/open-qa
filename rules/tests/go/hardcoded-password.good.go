package conf

import "os"

func Connect() string {
	dbPassword := os.Getenv("DB_PASSWORD")
	user := "admin"
	return user + ":" + dbPassword
}
