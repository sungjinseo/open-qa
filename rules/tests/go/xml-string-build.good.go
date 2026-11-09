package doc

import "encoding/xml"

type User struct {
	Name string `xml:"name"`
}

func Build(name string) ([]byte, error) {
	return xml.Marshal(User{Name: name})
}
