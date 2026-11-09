package proxy

import (
	"io"
	"net/http"
)

type store struct{}

func (store) Get(id string) string { return id }

func assertAllowedHost(u string) string { return "https://docs.example.com/" }

func Lookup(w http.ResponseWriter, r *http.Request) {
	var cache store
	w.Write([]byte(cache.Get(r.URL.Query().Get("id"))))
}

func Fetch(w http.ResponseWriter, r *http.Request) {
	target := assertAllowedHost(r.URL.Query().Get("url"))
	resp, _ := http.Get(target)
	io.Copy(w, resp.Body)
}
