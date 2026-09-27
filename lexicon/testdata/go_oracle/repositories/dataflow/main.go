package dataflow

type Box struct {
	Field int
}

const Constant = 3

func consume(value int) int {
	return value
}

func run(value int, box *Box) int {
	local := value
	local += Constant
	local++
	box.Field = local
	{
		value := local
		local = consume(value)
	}
	return local + value
}
