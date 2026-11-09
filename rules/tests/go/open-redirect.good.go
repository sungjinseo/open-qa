package auth

import "net/http"

func safeRedirectTarget(next string) string {
	if next == "/home" || next == "/profile" {
		return next
	}
	return "/home"
}

func AfterLogin(w http.ResponseWriter, r *http.Request) {
	next := r.URL.Query().Get("next")
	http.Redirect(w, r, safeRedirectTarget(next), http.StatusFound)
}
