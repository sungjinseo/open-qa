package work

import "fmt"

func Run(items []string) {
	for i := range items {
		go func() {
			fmt.Println(i)
		}()
	}
}
