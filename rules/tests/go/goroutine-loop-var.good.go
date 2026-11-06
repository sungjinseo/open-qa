package work

import "fmt"

func Run(items []string) {
	for i := range items {
		go func(n int) {
			fmt.Println(n)
		}(i)
	}
}
