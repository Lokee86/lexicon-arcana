# Detekt bounded-completion prompt experiment

Task: `detekt-cli-gradle-plugin-divergence`  
Model: `gpt-5.6-sol`, High reasoning, Fast  
Condition: raw Lexicon + Arcana  
Pinned Detekt revision: `f9e1d5cc239ab740ce499b1edb36b872012648e2`

The exact completion-bounded skill wording used for this condition is frozen in [`SKILL.md`](SKILL.md). The rerun script reads that snapshot rather than the shared benchmark skill, so later skill changes do not alter this experiment.

## Result

| Metric | Original L+A | Evidence-bounded prompt | Completion-bounded prompt |
|---|---:|---:|---:|
| Started items | 22 | 24 | **19** |
| Runtime | 533.0s | 676.7s | **300.6s** |
| Total input | 2.144M | 2.293M | **1.472M** |
| Fresh input | 381.2k | 232.4k | **112.9k** |
| Cached input | 1.763M | 2.060M | **1.359M** |
| Output | 15.5k | 18.5k | **13.2k** |
| Reasoning | 7.5k | 8.9k | **6.2k** |
| Grounding | valid | valid | **valid** |
| Manual quality | 8/8 | 8/8 | **8/8*** |

The stronger completion-surface wording materially improved efficiency. Relative to the original L+A run it reduced runtime by 43.6%, total input by 31.4%, fresh input by 70.4%, and cached input by 22.9%. Relative to the first prompt revision it reduced runtime by 55.6% and total input by 35.8%.

### Comparison with current plain exploration control

The more useful control for the overall architecture question is the current Codex Detekt plain-exploration run: the same frozen Detekt task and current GPT-5.6 Sol/High/Fast runtime, but with no Grimoire, Lexicon, or Arcana assistance.

| Metric | Plain exploration | L+A + completion-bound prompt | Change |
|---|---:|---:|---:|
| Started items | 25 | **19** | **-24.0%** |
| Runtime | 470.2s | **300.6s** | **-36.1%** |
| Total input | 1.906M | **1.472M** | **-22.8%** |
| Fresh input | 195.0k | **112.9k** | **-42.1%** |
| Cached input | 1.711M | **1.359M** | **-20.6%** |
| Output | 14.9k | **13.2k** | **-11.4%** |
| Reasoning | 7.7k | **6.2k** | **-19.7%** |
| Grounding | valid | **valid** | same |

The plain agent therefore spent materially more inference cycles and repository-reading context independently discovering the same diagnosis. The assisted run retained a strong grounded answer while using about one quarter fewer started items, one third less wall time, and 42% less fresh input. This strengthens the interpretation that prepared context is already valuable; the remaining inefficiency is primarily uncontrolled continuation after sufficient evidence has been found.

## Stopping behavior

The strict stopping target was not met.

At item 6 the model stated that the central asymmetry was already verified: CLI uses explicit `--plugins`, while Gradle omits that argument and merges rule-set JARs into its execution classpath. It then continued into Git history at item 7 despite the prompt explicitly telling it not to broaden into history. Later commands inspected additional core/configuration consequences, documentation, functional-test plumbing, and build/test surfaces. The run finished at 19 started items, not the hoped-for approximately 6–12.

This is therefore a mixed but useful result:

- Prompt-level completion wording can substantially reduce information gathering and inference-loop amplification.
- It does not reliably enforce the task-completion boundary.
- The model can explicitly acknowledge the stopping rule and then violate it immediately once it sees another plausible avenue of investigation.

That is strong empirical support for moving the deterministic stopping boundary into the framework: bounded inference units should ask one completion question at a time, and a model that says it is unresolved should name the specific missing fact for the framework to schedule next.

*Quality is recorded as 8/8 for apples-to-apples consistency with the existing manual Detekt scoring precedent. The answer preserves the same correct ownership, classloader-cache, convergence, fix-boundary, and verification conclusions. A stricter literal interpretation of the rubric's `failure_modes` wording would require revisiting the prior Detekt 8/8 scores as well, rather than applying a new standard only to this run.
