package role

import "slices"

type Roles struct {
	names []string
}

func (r *Roles) SetNames(names []string) {
	r.names = slices.Clone(names)
}
