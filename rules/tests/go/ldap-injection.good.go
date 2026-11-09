package dir

import (
	"github.com/go-ldap/ldap/v3"
)

func Find(uid string) *ldap.SearchRequest {
	filter := "(uid=" + ldap.EscapeFilter(uid) + ")"
	return ldap.NewSearchRequest("dc=example,dc=com", ldap.ScopeWholeSubtree, ldap.NeverDerefAliases, 0, 0, false,
		filter, []string{"cn"}, nil)
}
