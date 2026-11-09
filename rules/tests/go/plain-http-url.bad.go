package client

import "net/http"

func Users() (*http.Response, error) {
	return http.Get("http://api.example.com/v1/users")
}
