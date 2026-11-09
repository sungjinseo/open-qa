package conv

import "strconv"

func Port(s string) (uint16, error) {
	n, err := strconv.Atoi(s)
	if err != nil {
		return 0, err
	}
	return uint16(n), nil
}
