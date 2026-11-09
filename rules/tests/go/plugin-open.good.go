package ext

import "plugin"

func Load() (*plugin.Plugin, error) {
	return plugin.Open("/opt/app/plugins/audit.so")
}
