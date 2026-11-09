package token

import "github.com/golang-jwt/jwt/v5"

func Claims(raw string) (jwt.Claims, error) {
	t, _, err := jwt.NewParser().ParseUnverified(raw, jwt.MapClaims{})
	if err != nil {
		return nil, err
	}
	return t.Claims, nil
}
