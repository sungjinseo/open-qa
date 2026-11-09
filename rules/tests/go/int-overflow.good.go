package conv

import (
	"errors"
	"strconv"
)

func Port(s string) (uint16, error) {
	n, err := strconv.ParseUint(s, 10, 16)
	if err != nil {
		return 0, errors.New("invalid port")
	}
	return uint16(n), nil
}
