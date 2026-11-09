package hash

import "crypto/sha512"

func Sum(b []byte) []byte {
	h := sha512.New()
	h.Write(b)
	return h.Sum(nil)
}
