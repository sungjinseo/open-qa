package api

import (
	"log"
	"net/http"
)

func Handle(w http.ResponseWriter, r *http.Request) {
	if err := work(); err != nil {
		log.Printf("work failed: %v", err)
		http.Error(w, "internal error", http.StatusInternalServerError)
	}
}

func work() error { return nil }
