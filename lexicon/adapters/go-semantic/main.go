package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"time"
)

func main() {
	version := flag.Uint("protocol-version", 0, "private Go semantic protocol version")
	expectedHelperVersion := flag.String("helper-version", "", "expected private Go semantic helper version")
	showVersion := flag.Bool("version", false, "print helper version")
	flag.Parse()
	if *showVersion {
		fmt.Printf("lexicon-go-semantic %s\n", helperVersion)
		return
	}
	if uint32(*version) != protocolVersion {
		fmt.Fprintf(os.Stderr, "unsupported protocol version %d\n", *version)
		os.Exit(2)
	}
	if err := validateHelperVersion(*expectedHelperVersion); err != nil {
		fmt.Fprintln(os.Stderr, err)
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
	if len(value.Modules) > 0 {
		if err := requireGoToolchain(); err != nil {
			fmt.Fprintf(os.Stderr, "Go semantic helper runtime unavailable: %v\n", err)
			os.Exit(1)
		}
	}
	profiling := performanceEnabled()
	var result response
	var profile performanceProfile
	if profiling {
		result, profile, err = scanStructuralProfiled(value)
	} else {
		result, err = scanStructural(value)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "structural analysis failed: %v\n", err)
		os.Exit(1)
	}
	var encodingStarted time.Time
	if profiling {
		encodingStarted = time.Now()
	}
	if err := json.NewEncoder(os.Stdout).Encode(result); err != nil {
		fmt.Fprintf(os.Stderr, "encode semantic response: %v\n", err)
		os.Exit(1)
	}
	if profiling {
		profile.emit(time.Since(encodingStarted), len(result.Records))
	}
}
