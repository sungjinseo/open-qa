package doc

import (
	"encoding/xml"
	"fmt"
)

type User struct {
	Name string `xml:"name"`
}

func Build(name string) (User, error) {
	var u User
	err := xml.Unmarshal([]byte(fmt.Sprintf("<user><name>%s</name></user>", name)), &u)
	return u, err
}
