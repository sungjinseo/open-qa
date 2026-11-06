package enc

import (
	"crypto/aes"
	"crypto/cipher"
)

func New(key []byte) (cipher.Block, error) {
	return aes.NewCipher(key)
}
