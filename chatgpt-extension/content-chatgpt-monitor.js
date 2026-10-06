// Observer serial tracker — lightweight WeakMap, never touches observer internals.
const _observerSerial = typeof WeakMap !== 'undefined' ? new WeakMap() : null;
let _observerCounter = 0;
function _recSerial(obs) {
  if (!obs || !_observerSerial) return null;
  if (!_observerSerial.has(obs)) _observerSerial.set(obs, ++_observerCounter);
  return _observerSerial.get(obs);
}
function _debugEnabled() { try { return localStorage.getItem('chatcmd-debug') === '1'; } catch { return false; } }

globalThis.ChatCmdMonitor = Object.freeze({ create(api) {
  const { assistantNodes, latestMessageText, findStopButton, findThreadError, clickStopButton } = globalThis.ChatCmdConversationDom;
  // previousAnswer: text returned by the previous waitForAssistant call in this tool-loop turn.
  // Passing it prevents the recorder-inactive+hasTurn path from returning a stale answer that
  // was already consumed by the previous turn.
  return async function waitForAssistant(previousCount, requestId, submittedContent, previousAnswer = '') {
  let baselineCount = previousCount;
  let lastText = '';
  let stableSince = 0;
  let lastActivityAt = Date.now();
  let lastStateCheckAt = 0;
  let lastCompletionPingAt = 0;
  let lastRequestState = api.unknownRequestState();
  let observedProgress = false;
  const startedAt = Date.now();
  const isSubagent = requestId.startsWith('subagent:');
  let deadlineAt = startedAt + (isSubagent ? 30 : 10) * 60_000;
  // ── Diagnostic (throttled: only log when key state changes; gated by chatcmd-debug=1) ─────
  let _diagKey = '';
  function _diag(label, extra) {
    if (!_debugEnabled()) return;
    try {
      const _nodes = assistantNodes();
      const _latest = _nodes.at(-1);
      const _rec = api.activeRequest?.observer;
      const safeAttr = (el, attr) => (typeof el?.getAttribute === 'function' ? el.getAttribute(attr) : null);
      const nodeId = safeAttr(_latest, 'data-message-id') || safeAttr(_latest?.closest?.('[data-message-id]'), 'data-message-id') || safeAttr(_latest, 'data-testid') || '(none)';
      const scanText = _rec ? _rec.answer : (typeof _latest?.innerText === 'string' ? _latest.innerText.trim() : (_latest?.textContent?.trim() || ''));
      const textLen = scanText.length;
      const stableMs = stableSince ? Date.now() - stableSince : 0;
      const settleMs = _rec ? 4_000 : api.RAW_BUBBLE_STABILITY_MS;
      const hasStop = Boolean(findStopButton());
      const hasComposer = Boolean(api.findComposer());
      const hasErr = Boolean(findThreadError());
      const recHasTurn = _rec ? _rec.hasTurn : null;
      const recActive = _rec ? _rec.active : null;
      const hasPending = Boolean(globalThis.ChatCmdToolBridge?.hasPendingToolCalls(scanText));
      const execIds = globalThis.ChatCmdToolBridge?._executedCallIds ? [...globalThis.ChatCmdToolBridge._executedCallIds] : '(not exported)';
      const key = `${label}|${nodeId}|${Math.floor(textLen / 20)}|${Math.floor(stableMs / 512)}|${hasStop}|${hasComposer}|${hasErr}|${hasPending}|${recHasTurn}`;
      if (key === _diagKey) return;
      _diagKey = key;
      console.log('[ChatCMD Monitor]', label, {
        requestId, recSerial: _recSerial(_rec), nodeId, nodeCount: _nodes.length, baselineCount,
        textLen, stableMs, settleMs, stableEnough: stableMs >= settleMs,
        hasStopButton: hasStop, hasComposer, hasThreadError: hasErr,
        hasPendingToolCalls: hasPending, executedCallIds: execIds,
        usingRecorder: Boolean(_rec && recActive), recorderActive: recActive, recorderHasTurn: recHasTurn,
        hasNewAssistantText: (_rec ? recHasTurn : _nodes.length > baselineCount) && textLen > 0,
        textTail: scanText.slice(-120),
        ...extra,
      });
    } catch { /* never crash the loop due to diagnostics */ }
  }
  // ─────────────────────────────────────────────────────────────────────────────────────────
  while (Date.now() < deadlineAt) {
    if (!api.activeRequest || api.activeRequest.id !== requestId || api.activeRequest.resultReported) {
      _diag('early-exit:request-gone', { resultReported: api.activeRequest?.resultReported });
      return latestMessageText('assistant');
    }
    const now = Date.now();
    const nodes = assistantNodes();
    const latest = nodes.at(-1);
    const recorder = api.activeRequest.observer;  // always read fresh each iteration
    // effectiveRecorder may be nulled when the recorder has a stale answer (= previousAnswer),
    // which means it accidentally bound to an old user turn and is now stopped.
    // In that case we fall back to the DOM node-count approach for this iteration.
    let effectiveRecorder = recorder;
    if (recorder) {
      recorder.scan();
      if (!recorder.active && recorder.hasTurn) {
        const answer = recorder.answer;
        if (answer === previousAnswer) {
          // Observer accidentally bound to old content. Ignore it this iteration and use DOM
          // node-count instead. Also clamp baselineCount so we can detect a new node even
          // when ChatGPT's visible node count temporarily shrank below the original baseline.
          _diag('skip:stale-recorder', { answerLen: answer.length, recSerial: _recSerial(recorder) });
          if (_debugEnabled()) console.log('[ChatCMD Monitor] skip:stale-recorder', {
            requestId, answerLen: answer.length, recSerial: _recSerial(recorder), previousAnswerLen: previousAnswer.length,
          });
          effectiveRecorder = null;
          if (nodes.length < baselineCount) baselineCount = nodes.length;
        } else {
          _diag('exit:recorder-inactive+hasTurn', { answerLen: answer.length, recSerial: _recSerial(recorder) });
          return answer;
        }
      }
      if (effectiveRecorder) void recorder.flush(false, false);
    }
    const text = effectiveRecorder ? effectiveRecorder.answer : (latest?.innerText?.trim() || latest?.textContent?.trim() || '');
    const stopButton = findStopButton();
    const threadError = findThreadError();

    if (now - lastStateCheckAt > 800) {
      lastStateCheckAt = now;
      lastRequestState = await api.requestState(requestId);
      if (isSubagent && Number.isFinite(lastRequestState.deadlineAtMs)) deadlineAt = Math.min(lastRequestState.deadlineAtMs, startedAt + 24 * 60 * 60_000);
      if (lastRequestState.stopRequested && api.activeRequest?.id === requestId && !api.activeRequest.stopRequested) {
        api.activeRequest.stopRequested = true;
        clickStopButton();
        await api.delay(250);
        continue;
      }
    }

    if (api.activeRequest?.id === requestId && api.activeRequest.stopRequested) {
      clickStopButton();
      if (!findStopButton() && (lastRequestState.stopRequested || api.isTerminalRequestState(lastRequestState))) return text;
    }

    if (stopButton) {
      observedProgress = true;
      lastActivityAt = now;
    }
    const hasNewAssistantText = (effectiveRecorder ? effectiveRecorder.hasTurn : nodes.length > baselineCount) && Boolean(text);
    if (hasNewAssistantText) observedProgress = true;
    if (hasNewAssistantText && !threadError) {
      if (text !== lastText) {
        lastText = text;
        stableSince = now;
        lastActivityAt = now;
      } else if (!stableSince) {
        stableSince = now;
      }

      if (stopButton) stableSince = now;
      const stableMs = stableSince ? now - stableSince : 0;
      const settleMs = effectiveRecorder ? 4_000 : api.RAW_BUBBLE_STABILITY_MS;
      _diag('loop', {});
      if (!stopButton && stableMs >= settleMs && api.isTerminalRequestState(lastRequestState)) {
        _diag('exit:terminal+stable', { isTerminal: true });
        if (!effectiveRecorder || await effectiveRecorder.flush(true)) return text;
      }
      if (
        !stopButton && !threadError && api.findComposer() &&
        stableMs >= settleMs && now - lastCompletionPingAt >= api.COMPLETION_PING_INTERVAL_MS
      ) {
        const hasPending = globalThis.ChatCmdToolBridge?.hasPendingToolCalls(text);
        if (_debugEnabled()) {
          const parsedIds = globalThis.ChatCmdToolBridge?.parseToolCallIds?.(text) || '(not exported)';
          console.log('[ChatCMD Monitor] completion-check', {
            requestId, hasPending, parsedIds,
            textLen: text.length, textHead: text.slice(0, 200), textTail: text.slice(-200),
            stableMs, settleMs, usingRecorder: Boolean(effectiveRecorder),
          });
        }
        // If the bridge has pending tool calls, return text to the tool bridge loop
        // without reporting browser completion to the backend.
        if (hasPending) {
          if (!effectiveRecorder || await effectiveRecorder.flush(true)) {
            _diag('exit:hasPendingToolCalls', {});
            return text;
          }
        }
        lastCompletionPingAt = now;
        if (await api.reportBrowserCompletion(requestId, text)) {
          _diag('exit:browserCompletion', {});
          return text;
        }
      }
    } else {
      _diag('no-new-text', {});
    }

    if (!stopButton && api.findComposer()) {
      const idleMs = now - lastActivityAt;
      const reason = threadError && idleMs >= api.ERROR_INTERRUPT_GRACE_MS
        ? 'thread_error'
        : idleMs >= api.SILENT_RETRY_GRACE_MS && !hasNewAssistantText
          ? 'send_ready_without_final'
          : null;
      if (reason && api.AUTO_RETRY_ENABLED) {
        lastRequestState = await api.requestState(requestId);
        if (isSubagent && Number.isFinite(lastRequestState.deadlineAtMs)) deadlineAt = Math.min(lastRequestState.deadlineAtMs, startedAt + 24 * 60 * 60_000);
        if (!lastRequestState.known || lastRequestState.hasFinalResponse || !lastRequestState.active) {
          await api.delay(350);
          continue;
        }
        if ((api.activeRequest?.retryCount || 0) >= api.MAX_AUTO_RETRIES) {
          throw new Error(`ChatGPT still has not produced a final response after ${api.MAX_AUTO_RETRIES} automatic retries.`);
        }
        baselineCount = nodes.length;
        lastText = '';
        stableSince = 0;
        await api.retryPrompt(requestId, observedProgress ? api.INTERRUPTED_PROGRESS_PROMPT : submittedContent, reason, observedProgress);
        lastActivityAt = Date.now();
        await api.delay(650);
        continue;
      }
    }
    await api.delay(350);
  }
  throw new Error('Timed out waiting for a completion response from ChatGPT.');
};
} });
