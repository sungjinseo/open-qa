package upload

import (
	"net/http"
	"os"
)

func Save(w http.ResponseWriter, r *http.Request) {
	_, hdr, _ := r.FormFile("file")
	out, _ := os.Create("/data/uploads/" + hdr.Filename)
	defer out.Close()
}
