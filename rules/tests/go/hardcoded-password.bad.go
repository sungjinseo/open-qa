package conf

func DSN() string {
	dbPassword := "P@ssw0rd-prod"
	return "user:" + dbPassword + "@tcp(db:3306)/app"
}
