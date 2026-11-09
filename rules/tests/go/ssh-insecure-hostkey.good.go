package remote

import (
	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

func Config() (*ssh.ClientConfig, error) {
	cb, err := knownhosts.New("/home/deploy/.ssh/known_hosts")
	if err != nil {
		return nil, err
	}
	return &ssh.ClientConfig{User: "deploy", HostKeyCallback: cb}, nil
}
