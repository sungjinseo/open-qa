package panel

import "net/http"

func roleOf(r *http.Request) string { return "user" }

func Panel(w http.ResponseWriter, r *http.Request) {
	if roleOf(r) == "admin" {
		w.Write([]byte("admin panel"))
	}
}
