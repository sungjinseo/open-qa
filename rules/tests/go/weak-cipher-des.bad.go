package enc

import "crypto/des"

func Make(key []byte) error {
	_, err := des.NewCipher(key)
	return err
}
