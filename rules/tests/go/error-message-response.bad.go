package api

import "net/http"

func Handle(w http.ResponseWriter, r *http.Request) {
	if err := work(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func work() error { return nil }
