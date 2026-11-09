package main

import "net/http"

func requireAdmin(h http.HandlerFunc) http.HandlerFunc { return h }

func reset(w http.ResponseWriter, r *http.Request) { w.Write([]byte("reset")) }

func main() {
	http.HandleFunc("/admin/reset", requireAdmin(reset))
}
