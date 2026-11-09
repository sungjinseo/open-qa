package auth

import "net/http"

func AfterLogin(w http.ResponseWriter, r *http.Request) {
	next := r.URL.Query().Get("next")
	http.Redirect(w, r, next, http.StatusFound)
}
