package server

import "net/http"

func withCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "https://app.example.com")
		w.Header().Set("Vary", "Origin")
		next.ServeHTTP(w, r)
	})
}
