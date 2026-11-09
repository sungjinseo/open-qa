package hash

import "crypto/sha256"

func Sum(b []byte) [32]byte {
	return sha256.Sum256(b)
}
