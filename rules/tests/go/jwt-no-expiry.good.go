package auth

import (
	"time"

	"github.com/golang-jwt/jwt/v5"
)

func Issue(userID string, key []byte) (string, error) {
	t := jwt.NewWithClaims(jwt.SigningMethodHS256, jwt.MapClaims{"sub": userID, "exp": time.Now().Add(15 * time.Minute).Unix()})
	return t.SignedString(key)
}
