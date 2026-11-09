package conf

import "os"

func Save(path string, data []byte) error {
	return os.WriteFile(path, data, 0o777)
}
