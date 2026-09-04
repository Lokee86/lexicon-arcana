# Detekt plain bounded-completion prompt experiment

Task: `detekt-cli-gradle-plugin-divergence`  
Model: `gpt-5.6-sol`, High reasoning, Fast  
Condition: plain repository exploration; no Grimoire, Lexicon, Arcana, or CBM  
Pinned Detekt revision: `f9e1d5cc239ab740ce499b1edb36b872012648e2`

## Prompt variant

The ordinary benchmark prompt was augmented with an explicit completion policy: once the agent can identify the likely owner, explain the causal divergence, define the smallest correct fix boundary, and provide a sufficient source-supported verification plan, it should stop. It was told not to broaden into related paths, history, documentation, alternative hypotheses, test plumbing, or extra verification merely to increase confidence, and not to seek redundant confirmation of an already-supported conclusion.

## Result

| Metric | Current plain control | Bounded-completion plain | Change |
| --- | ---: | ---: | ---: |
| Started items | 25 | **21** | **-16.0%** |
| Runtime | **470.2s** | 490.6s | **+4.3%** |
| Total input | 1.906M | **1.219M** | **-36.1%** |
| Fresh input | 195.0k | **146.3k** | **-25.0%** |
| Cached input | 1.711M | **1.072M** | **-37.3%** |
| Output | 14.9k | **14.0k** | **-5.5%** |
| Reasoning | **7.7k** | 8.4k | **+8.5%** |
| Grounding | valid | **valid** | same |

The stronger wording reduced repository calls and context consumption materially, especially cached replay, but did not improve latency. Runtime increased slightly and reasoning output increased.

## Trace behavior

After four repository commands, item 6 explicitly stated the central divergence: Gradle resolves `detektPlugins` but puts those JARs on detekt's execution classpath instead of passing the CLI's real `--plugins` argument, while the standalone CLI passes plugin paths into core's dedicated plugin-loader path.

The agent then made **17 additional repository commands** before answering. Those calls checked sibling task types, functional-test fixtures, sample extensions, documentation, configuration resources, existing cache tests, path splitting, config export, tooling extension semantics, and additional classloader rationale. One command failed and was retried, but that accounts for only one extra call.

Thus roughly **81% of the run's shell calls occurred after the agent had already explicitly identified the central causal divergence**. This is nearly the same qualitative pathology as the original plain run: the model finds the important evidence early and then spends most of the investigation increasing confidence and completing adjacent verification detail.

The stronger wording did improve the *shape* of that continuation. The extra work stayed more tightly coupled to the requested fix boundary and verification plan, and total input fell 36%. But it did not create a reliable semantic stop boundary.

## Interpretation

This run strengthens the current bounded-inference hypothesis. Prompting can narrow and cheapen an epistemic reassurance loop, but the model still retains an open-ended mandate to decide whether it is sufficiently sure. The framework should instead bound individual inference questions and make continuation explicit through a named unresolved fact.

Raw trace, answer, grounding validation, and machine summary are retained in this result directory.
