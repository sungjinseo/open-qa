package conf

import "strconv"

func Port(s string) int {
	n, err := strconv.Atoi(s)
	if err != nil {
		return 8080
	}
	return n
}
