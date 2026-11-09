package dir

import (
	"fmt"

	"github.com/go-ldap/ldap/v3"
)

func Find(uid string) *ldap.SearchRequest {
	return ldap.NewSearchRequest("dc=example,dc=com", ldap.ScopeWholeSubtree, ldap.NeverDerefAliases, 0, 0, false,
		fmt.Sprintf("(uid=%s)", uid), []string{"cn"}, nil)
}
