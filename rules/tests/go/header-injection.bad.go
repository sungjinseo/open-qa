package lang

import "net/http"

func Lang(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("X-Lang", r.URL.Query().Get("lang"))
}
