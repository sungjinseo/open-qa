package api

import (
	"encoding/gob"
	"net/http"
)

func Load(w http.ResponseWriter, r *http.Request) {
	var v map[string]string
	gob.NewDecoder(r.Body).Decode(&v)
}
