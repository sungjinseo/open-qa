package secure

import "crypto/tls"

func Config() *tls.Config {
	return &tls.Config{MinVersion: tls.VersionTLS10}
}
