import * as ts from "typescript";
import { emitCallbackEdges, emitCallableAliases, propagateArguments } from "./call-flow";
import { multiTargetCallReason, unresolvedCallReason } from "./call-classification";
import { callTargetName, relativeSourcePath, sameSet, type ParameterTargets } from "./call-shared";
import { buildDispatchIndex, resolveCallTargets } from "./call-targets";
import { expressionText, spanFor } from "./contract";
import type { FactStore, PendingCall } from "./model";

type ResolvedCalls = Map<PendingCall, Set<string>>;

export function resolveCalls(facts: FactStore, checker: ts.TypeChecker): void {
  const parameterTargets: ParameterTargets = new Map();
  const resolved: ResolvedCalls = new Map();
  const dispatchIndex = buildDispatchIndex(facts);
  for (let iteration = 0; iteration < 16; iteration += 1) {
    let changed = false;
    for (const call of facts.calls) {
      const targets = resolveCallTargets(facts, checker, call, parameterTargets, dispatchIndex);
      if (!sameSet(targets, resolved.get(call))) {
        resolved.set(call, targets);
        changed = true;
      }
    }
    for (const call of facts.calls) {
      changed = propagateArguments(
        facts,
        checker,
        call,
        resolved.get(call) ?? new Set(),
        parameterTargets,
      ) || changed;
    }
    if (!changed) break;
  }

  emitCallableAliases(facts, checker, parameterTargets);
  for (const call of facts.calls) {
    emitCall(facts, checker, call, resolved.get(call) ?? new Set(), parameterTargets);
    emitCallbackEdges(facts, checker, call, parameterTargets);
  }
}

function emitCall(
  facts: FactStore,
  checker: ts.TypeChecker,
  call: PendingCall,
  targets: Set<string>,
  parameterTargets: ParameterTargets,
): void {
  const recordSpan = spanFor(call.expression, call.sourceFile, relativeSourcePath(facts, call.sourceFile));
  if (targets.size === 1) {
    facts.addEdge(call.source, [...targets][0], "calls", recordSpan);
    return;
  }
  if (targets.size > 1) {
    for (const target of [...targets].sort()) facts.addEdge(call.source, target, "possible-calls", recordSpan);
    facts.addUnresolved(
      call.source,
      "calls",
      expressionText(call.expression, call.sourceFile),
      multiTargetCallReason(checker, call),
      recordSpan,
      callTargetName(call),
    );
    return;
  }
  facts.addUnresolved(
    call.source,
    "calls",
    expressionText(call.expression, call.sourceFile),
    unresolvedCallReason(facts, checker, call, parameterTargets),
    recordSpan,
    callTargetName(call),
  );
}
