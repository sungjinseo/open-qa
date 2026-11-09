package lang

import (
	"net/http"
	"net/url"
)

func Lang(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("X-Lang", url.QueryEscape(r.URL.Query().Get("lang")))
}
