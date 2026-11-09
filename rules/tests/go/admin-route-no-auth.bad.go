package main

import "net/http"

func main() {
	http.HandleFunc("/admin/reset", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("reset"))
	})
}
