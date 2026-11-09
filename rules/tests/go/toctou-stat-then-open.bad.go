package store

import "os"

func Save(path string, data []byte) error {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return os.WriteFile(path, data, 0o600)
	}
	return nil
}
