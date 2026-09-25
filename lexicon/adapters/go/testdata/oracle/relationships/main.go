package relationships

type Contract interface {
	Run() int
}

type Extended interface {
	Contract
	Stop()
}

type Base struct{}

func (Base) Run() int { return 1 }

type Embedded struct {
	Base
}

type Direct struct{}

func (Direct) Run() int { return 2 }

func invoke(contract Contract) int {
	return contract.Run()
}

func caller() int {
	return invoke(Embedded{}) + invoke(Direct{})
}
