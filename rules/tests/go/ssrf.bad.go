package proxy

import (
	"io"
	"net/http"
)

func Fetch(w http.ResponseWriter, r *http.Request) {
	target := r.URL.Query().Get("url")
	resp, _ := http.Get(target)
	io.Copy(w, resp.Body)
}
