package enc

import (
	"crypto/cipher"
	"crypto/des"
)

func New(key []byte) (cipher.Block, error) {
	return des.NewCipher(key)
}
