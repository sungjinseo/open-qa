package gate

import (
	"net"
	"strings"
)

func Internal(ip string) bool {
	names, err := net.LookupAddr(ip)
	return err == nil && len(names) > 0 && strings.HasSuffix(names[0], ".corp.local.")
}
