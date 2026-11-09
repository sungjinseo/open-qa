package token

import "github.com/golang-jwt/jwt/v5"

func Claims(raw string, key []byte) (jwt.Claims, error) {
	t, err := jwt.Parse(raw, func(*jwt.Token) (any, error) { return key, nil }, jwt.WithValidMethods([]string{"HS256"}))
	if err != nil {
		return nil, err
	}
	return t.Claims, nil
}
