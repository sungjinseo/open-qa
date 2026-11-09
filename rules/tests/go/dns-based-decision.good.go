package gate

import "strings"

func Internal(ip string) bool {
	return strings.HasPrefix(ip, "10.")
}
