package role

type Roles struct {
	names []string
}

func (r *Roles) SetNames(names []string) {
	r.names = names
}
