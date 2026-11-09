package page

import (
	"net/http"
	"text/template"
)

func Render(w http.ResponseWriter, r *http.Request) {
	t, _ := template.New("p").Parse("<h1>{{.Title}}</h1>")
	t.Execute(w, map[string]string{"Title": r.FormValue("title")})
}
