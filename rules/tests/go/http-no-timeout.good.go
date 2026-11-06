package main

import (
	"net/http"
	"time"
)

func main() {
	srv := &http.Server{Addr: ":8080", ReadHeaderTimeout: 5 * time.Second}
	srv.ListenAndServe()
}
