package conf

import "errors"

func Must(s string) (string, error) {
	if s == "" {
		return "", errors.New("empty")
	}
	return s, nil
}
