package conf

import "os"

func DSN() string {
	dbPassword := os.Getenv("DB_PASSWORD")
	return "user:" + dbPassword + "@tcp(db:3306)/app"
}

const envPassword = "DB_PASSWORD"
const passwordField = "password"
const secretLabel = "Secret"
