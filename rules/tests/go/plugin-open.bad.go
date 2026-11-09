package ext

import "plugin"

func Load(path string) (*plugin.Plugin, error) {
	return plugin.Open(path)
}
