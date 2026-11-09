package conf

func Must(s string) string {
	if s == "" {
		panic("empty")
	}
	return s
}
