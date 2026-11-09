package role

import "slices"

type Roles struct {
	names []string
}

func (r *Roles) Names() []string {
	return slices.Clone(r.names)
}
