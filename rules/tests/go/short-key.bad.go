package keys

import (
	"crypto/rand"
	"crypto/rsa"
)

func New() (*rsa.PrivateKey, error) {
	return rsa.GenerateKey(rand.Reader, 1024)
}
