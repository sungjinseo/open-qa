package ops

import "os/exec"

func Ping(host string) error {
	return exec.Command("ping", "-c", "1", host).Run()
}
