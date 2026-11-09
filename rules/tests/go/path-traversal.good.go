package files

import (
	"net/http"
	"os"
	"path/filepath"
)

func Download(w http.ResponseWriter, r *http.Request) {
	name := filepath.Base(r.URL.Query().Get("file"))
	data, _ := os.ReadFile(filepath.Join("/data/uploads", name))
	w.Write(data)
}
