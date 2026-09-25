package service

import "example.com/oracle/parallel/worker"

func Start() int {
	value := &worker.Fast{}
	return worker.Execute(value)
}
