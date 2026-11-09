package token

import "github.com/golang-jwt/jwt/v5"

func Issue(uid string) (string, error) {
	t := jwt.NewWithClaims(jwt.SigningMethodHS256, jwt.MapClaims{"uid": uid})
	return t.SignedString([]byte("my-super-secret"))
}

func keyFunc(t *jwt.Token) (interface{}, error) {
	return []byte("my-super-secret"), nil
}
