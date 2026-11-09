package upload

import (
	"net/http"
	"os"
	"path/filepath"
)

func Save(w http.ResponseWriter, r *http.Request) {
	out, _ := os.Create(filepath.Join("/data/uploads", newID()+".dat"))
	defer out.Close()
}

func newID() string { return "generated" }
