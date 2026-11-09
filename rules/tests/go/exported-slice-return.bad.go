package role

type Roles struct {
	names []string
}

func (r *Roles) Names() []string {
	return r.names
}
