package hash

import "crypto/sha512"

func Sum(b []byte) [64]byte {
	return sha512.Sum512(b)
}
