package job

func Run(f func()) {
	defer func() {
		recover()
	}()
	f()
}
