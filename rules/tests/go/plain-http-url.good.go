package client

import "net/http"

func Users() (*http.Response, error) {
	return http.Get("https://api.example.com/v1/users")
}
