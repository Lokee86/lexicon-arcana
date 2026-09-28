package higher

func first()  {}
func second() {}
func target() {}

func apply(callback func()) {
	callback()
}

func caller(flag bool) {
	callback := first
	if flag {
		callback = second
	}
	apply(callback)

	f := target
	f()

	captured := 1
	closure := func() {
		_ = captured
		target()
	}
	closure()
	func() { target() }()
}
