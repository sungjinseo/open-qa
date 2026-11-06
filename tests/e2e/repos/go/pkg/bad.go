package pkg

import "strconv"

func Port(s string) int {
	n, _ := strconv.Atoi(s)
	return n
}
