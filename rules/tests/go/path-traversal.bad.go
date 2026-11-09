package files

import (
	"net/http"
	"os"
)

func Download(w http.ResponseWriter, r *http.Request) {
	name := r.URL.Query().Get("file")
	data, _ := os.ReadFile("/data/uploads/" + name)
	w.Write(data)
}
