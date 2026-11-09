package files

import "os"

func readOne(p string) error {
	f, err := os.Open(p)
	if err != nil {
		return err
	}
	defer f.Close()
	return nil
}

func ReadAll(paths []string) {
	for _, p := range paths {
		_ = readOne(p)
	}
}
