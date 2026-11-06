package shell

import "os/exec"

func Ping(host string) error {
	return exec.Command("sh", "-c", "ping -c 1 "+host).Run()
}
