package page

import (
	"net/http"
	"text/template"
)

func Render(w http.ResponseWriter, r *http.Request) {
	t, _ := template.New("p").Parse("<h1>" + r.FormValue("title") + "</h1>")
	t.Execute(w, nil)
}
