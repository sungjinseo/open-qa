package report

import "log/slog"

func Show(s string) {
	slog.Info("report", "value", s)
}
