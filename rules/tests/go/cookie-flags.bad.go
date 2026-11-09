package session

import "net/http"

func Set(w http.ResponseWriter, sid string) {
	http.SetCookie(w, &http.Cookie{Name: "sid", Value: sid})
}
