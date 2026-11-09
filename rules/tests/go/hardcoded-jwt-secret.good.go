package token

import (
	"os"

	"github.com/golang-jwt/jwt/v5"
)

func Issue(uid string) (string, error) {
	t := jwt.NewWithClaims(jwt.SigningMethodHS256, jwt.MapClaims{"uid": uid})
	return t.SignedString([]byte(os.Getenv("JWT_SECRET")))
}

func defaultSalt() ([]byte, error) {
	return []byte("static-salt"), nil
}
