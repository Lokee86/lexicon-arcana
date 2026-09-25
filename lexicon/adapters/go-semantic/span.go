package main

import (
	"go/token"
	"strconv"
)

func sourceSpan(set *token.FileSet, start, end token.Pos) span {
	begin := set.PositionFor(start, false)
	finish := set.PositionFor(end, false)
	return span{
		StartLine: uint32(begin.Line), StartColumn: uint32(begin.Column),
		EndLine: uint32(finish.Line), EndColumn: uint32(finish.Column),
	}
}

func itoa(value int) string {
	return strconv.Itoa(value)
}
