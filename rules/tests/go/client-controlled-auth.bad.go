package panel

import "net/http"

func Panel(w http.ResponseWriter, r *http.Request) {
	if r.Header.Get("X-Is-Admin") == "true" {
		w.Write([]byte("admin panel"))
	}
}
