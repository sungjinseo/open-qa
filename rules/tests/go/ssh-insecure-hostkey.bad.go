package remote

import "golang.org/x/crypto/ssh"

func Config() *ssh.ClientConfig {
	return &ssh.ClientConfig{User: "deploy", HostKeyCallback: ssh.InsecureIgnoreHostKey()}
}
