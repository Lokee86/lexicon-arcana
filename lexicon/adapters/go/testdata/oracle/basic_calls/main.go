package basic

import (
	"fmt"

	"example.com/oracle/basic/internal/sub"
)

type Widget int

func helper()    {}
func recursive() { recursive() }

func caller() {
	helper()
	sub.Function()
	var value sub.Thing
	value.Method()
	Widget(0).Local()
	_ = len([]int{})
	fmt.Println("external")
	var dynamic func()
	dynamic()
}

func (Widget) Local() {}
