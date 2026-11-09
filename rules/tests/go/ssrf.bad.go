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

func ViaParam(c *http.Client, r *http.Request) {
	c.Get(r.URL.Query().Get("url"))
}

func ViaLocal(r *http.Request) {
	client := &http.Client{}
	client.Get(r.URL.Query().Get("url"))
}

func ViaVar(r *http.Request) {
	var c http.Client
	c.Get(r.URL.Query().Get("url"))
}
