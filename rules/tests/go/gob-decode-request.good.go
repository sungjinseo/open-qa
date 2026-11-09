package api

import (
	"encoding/json"
	"net/http"
)

func Load(w http.ResponseWriter, r *http.Request) {
	var v map[string]string
	json.NewDecoder(http.MaxBytesReader(w, r.Body, 1<<20)).Decode(&v)
}
