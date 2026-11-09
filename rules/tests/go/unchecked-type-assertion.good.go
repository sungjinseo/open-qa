package conv

func Name(v any) string {
	s, ok := v.(string)
	if !ok {
		return ""
	}
	return s
}
