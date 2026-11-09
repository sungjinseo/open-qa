package client

import "crypto/tls"

func Config() *tls.Config {
	return &tls.Config{MinVersion: tls.VersionTLS12}
}
