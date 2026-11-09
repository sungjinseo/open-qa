package hash

import "crypto/sha1"

func Sum(b []byte) [20]byte {
	return sha1.Sum(b)
}
