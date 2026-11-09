package conf

import "os"

func DSN() string {
	dbPassword := os.Getenv("DB_PASSWORD")
	return "user:" + dbPassword + "@tcp(db:3306)/app"
}

const envPassword = "DB_PASSWORD"
const passwordField = "password"
const secretLabel = "Secret"
const passwordKey = "auth.password"
const passwordRule = "password-required"
const passwordCol = "password_field"
const passwordPrompt = "Password:"
const passwordMsg = "비밀번호를입력하세요"
