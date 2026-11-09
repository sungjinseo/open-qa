package token

import (
	"crypto/rand"
	"math/big"
)

func Pin() (int64, error) {
	n, err := rand.Int(rand.Reader, big.NewInt(10000))
	if err != nil {
		return 0, err
	}
	return n.Int64(), nil
}
