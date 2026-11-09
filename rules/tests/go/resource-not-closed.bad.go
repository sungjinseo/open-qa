package files

import (
	"io"
	"os"
)

func Read(path string) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	return io.ReadAll(f)
}
