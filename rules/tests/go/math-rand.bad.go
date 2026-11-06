package token

import "math/rand"

func Pin() int {
	return rand.Intn(10000)
}
