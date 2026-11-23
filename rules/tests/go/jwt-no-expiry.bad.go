package auth

import "github.com/golang-jwt/jwt/v5"

func Issue(userID string, key []byte) (string, error) {
	t := jwt.NewWithClaims(jwt.SigningMethodHS256, jwt.MapClaims{"sub": userID})
	return t.SignedString(key)
}
