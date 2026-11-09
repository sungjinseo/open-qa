package conf

import "os"

func Load(path string) []byte {
	data, err := os.ReadFile(path)
	if err != nil {
	}
	return data
}
