package page

import (
	"html/template"
	"net/http"
)

func Render(w http.ResponseWriter, r *http.Request) {
	t := template.Must(template.New("p").Parse("{{.Body}}"))
	t.Execute(w, map[string]any{"Body": r.FormValue("body")})
}
