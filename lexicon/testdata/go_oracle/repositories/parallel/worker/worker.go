package worker

type Runner interface {
	Run() int
}

type Fast struct {
	Value int
}

func (f *Fast) Run() int {
	f.Value++
	return f.Value
}

func Execute(r Runner) int {
	return r.Run()
}
