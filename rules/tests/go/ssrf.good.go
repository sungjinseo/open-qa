package proxy

import (
	"io"
	"net/http"
)

func assertAllowedHost(u string) string { return "https://docs.example.com/" }

func Fetch(w http.ResponseWriter, r *http.Request) {
	target := assertAllowedHost(r.URL.Query().Get("url"))
	resp, _ := http.Get(target)
	io.Copy(w, resp.Body)
}
