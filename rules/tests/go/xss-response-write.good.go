package hello

import (
	"fmt"
	"html"
	"net/http"
)

func Hello(w http.ResponseWriter, r *http.Request) {
	name := html.EscapeString(r.URL.Query().Get("name"))
	fmt.Fprintf(w, "<h1>Hello %s</h1>", name)
}
