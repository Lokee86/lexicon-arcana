package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
)

func main() {
	version := flag.Uint("protocol-version", 0, "private Go semantic protocol version")
	flag.Parse()
	if uint32(*version) != protocolVersion {
		fmt.Fprintf(os.Stderr, "unsupported protocol version %d\n", *version)
		os.Exit(2)
	}

	reader := bufio.NewReader(os.Stdin)
	payload, err := reader.ReadBytes('\n')
	if err != nil && len(payload) == 0 {
		fmt.Fprintf(os.Stderr, "read semantic request: %v\n", err)
		os.Exit(1)
	}
	value, err := decodeRequest(payload)
	if err != nil {
		fmt.Fprintf(os.Stderr, "decode semantic request: %v\n", err)
		os.Exit(1)
	}
	result, err := scanStructural(value)
	if err != nil {
		fmt.Fprintf(os.Stderr, "structural analysis failed: %v\n", err)
		os.Exit(1)
	}
	if err := json.NewEncoder(os.Stdout).Encode(result); err != nil {
		fmt.Fprintf(os.Stderr, "encode semantic response: %v\n", err)
		os.Exit(1)
	}
}
