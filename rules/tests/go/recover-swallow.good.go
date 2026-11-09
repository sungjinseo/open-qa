package job

import "log"

func Run(f func()) {
	defer func() {
		if r := recover(); r != nil {
			log.Printf("job panic: %v", r)
		}
	}()
	f()
}
