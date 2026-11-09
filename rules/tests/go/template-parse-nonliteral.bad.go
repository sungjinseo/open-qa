package page

import (
	"net/http"
	"text/template"
)

func Render(w http.ResponseWriter, r *http.Request) {
	t, _ := template.New("p").Parse("<h1>" + r.FormValue("title") + "</h1>")
	t.Execute(w, nil)
}

func RenderVar(w http.ResponseWriter, r *http.Request) {
	t := template.New("q")
	t, _ = t.Parse(r.FormValue("tpl"))
	t.Execute(w, nil)
}
