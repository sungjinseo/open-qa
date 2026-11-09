package page

import (
	"net/http"
	"net/url"
	"text/template"

	"github.com/google/uuid"
)

func Render(w http.ResponseWriter, r *http.Request) {
	t, _ := template.New("p").Parse("<h1>{{.Title}}</h1>")
	t.Execute(w, map[string]string{"Title": r.FormValue("title")})
}

func Addr(raw string) (*url.URL, error) {
	u, err := url.Parse(raw)
	id, _ := uuid.Parse(raw)
	_ = id
	return u, err
}
