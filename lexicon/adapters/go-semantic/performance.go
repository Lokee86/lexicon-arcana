package main

import (
	"fmt"
	"os"
	"strings"
	"time"
)

type performanceProfile struct {
	StructuralParsing time.Duration
	PackageLoad       time.Duration
	SemanticIndex     time.Duration
	Relationships     time.Duration
	CallsDataflow     time.Duration
	SSAVTA            time.Duration

	ParsedFiles       int
	LoadedPackages    int
	PeakLivePackages  int
	ProcessedModules  int
	TypedTargets      int
	TypedTypes        int
	RawCalls          int
	CompactedCalls    int
	RawDataflow       int
	CompactedDataflow int
}

type performanceCounter struct {
	name  string
	value int
}

func performanceEnabled() bool {
	value := strings.ToLower(strings.TrimSpace(os.Getenv("LEXICON_PERF")))
	switch value {
	case "", "0", "false", "off", "no":
		return false
	default:
		return true
	}
}

func (profile performanceProfile) emit(responseEncoding time.Duration, responseRecords int) {
	if !performanceEnabled() {
		return
	}
	emitPerformance("go.frontend.parse", profile.StructuralParsing,
		performanceCounter{"parsed_files", profile.ParsedFiles})
	emitPerformance("go.frontend.project_load", profile.PackageLoad,
		performanceCounter{"loaded_packages", profile.LoadedPackages},
		performanceCounter{"peak_live_packages", profile.PeakLivePackages},
		performanceCounter{"processed_modules", profile.ProcessedModules})
	semanticAnalysis := profile.SemanticIndex +
		profile.Relationships +
		profile.CallsDataflow +
		profile.SSAVTA
	emitPerformance("go.frontend.semantic_analysis", semanticAnalysis,
		performanceCounter{"typed_targets", profile.TypedTargets},
		performanceCounter{"typed_types", profile.TypedTypes},
		performanceCounter{"raw_call_observations", profile.RawCalls},
		performanceCounter{"compacted_calls", profile.CompactedCalls},
		performanceCounter{"raw_dataflow_records", profile.RawDataflow},
		performanceCounter{"compacted_dataflow", profile.CompactedDataflow})
	emitPerformance("go.frontend.observation_emit", responseEncoding,
		performanceCounter{"response_records", responseRecords})
}

func emitPerformance(stage string, elapsed time.Duration, counters ...performanceCounter) {
	fmt.Fprintf(os.Stderr, "[lexicon-perf] stage=%s elapsed_ms=%.3f",
		stage, float64(elapsed)/float64(time.Millisecond))
	for _, counter := range counters {
		fmt.Fprintf(os.Stderr, " %s=%d", counter.name, counter.value)
	}
	fmt.Fprintln(os.Stderr)
}
