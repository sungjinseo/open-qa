package pw

import "golang.org/x/crypto/bcrypt"

func Store(password string) ([]byte, error) {
	return bcrypt.GenerateFromPassword([]byte(password), 12)
}
