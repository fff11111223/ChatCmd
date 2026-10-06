/**
 * content-chatgpt-tool-bridge.js
 *
 * Loaded after content-chatgpt-monitor.js and before content-chatgpt.js.
 *
 * Responsibilities:
 *   1. Parse CHATCMD_TOOL_CALL blocks from ChatGPT assistant responses.
 *   2. First-layer call_id deduplication (JS Set).
 *   3. Request background execution via handleToolCall() (background-io.js).
 *   4. Receive result and format a CHATCMD_TOOL_RESULT block.
 *   5. Inject the result block into the composer and submit it so ChatGPT
 *      can continue the conversation.
 *
 * The monitor (content-chatgpt-monitor.js) defers completion reporting
 * to this bridge when hasPendingToolCalls() returns true.
 */
(() => {
  const _dbg = () => { try { return localStorage.getItem('chatcmd-debug') === '1'; } catch { return false; } };
  // ── Protocol patterns ──────────────────────────────────────────────────
  //
  // ChatGPT emits tool calls wrapped in a custom fenced-code block:
  //
  //   ```chatcmd_tool_call
  //   {"id":"call_abc","tool":"read_file","arguments":{"path":"..."}}
  //   ```
  //
  // A flat inline tag is also accepted for forward-compat:
  //
  //   [[CHATCMD_TOOL_CALL:{"id":"call_abc","tool":"read_file","arguments":{}}]]
  //
  // JS Set for first-layer deduplication.
  const executedCallIds = new Set();
  const inflight = new Map();

  // ── Parsing ────────────────────────────────────────────────────────────

  /**
   * Extract all tool-call descriptors from `text`.
   * Matches ```chatcmd_tool_call ... ``` or [[CHATCMD_TOOL_CALL:{...}]]
   * or raw JSON containing "tool" and "id" when wrapped in code blocks.
   */
  function parseToolCalls(text) {
    if (!text || typeof text !== 'string') return [];
    const calls = [];
    const seen = new Set();

    function tryAdd(raw) {
      try {
        const trimmed = String(raw || '').trim();
        let candidate = trimmed;
        if (!candidate.startsWith('{')) {
          const firstBrace = candidate.indexOf('{');
          const lastBrace = candidate.lastIndexOf('}');
          if (firstBrace !== -1 && lastBrace > firstBrace) {
            candidate = candidate.slice(firstBrace, lastBrace + 1);
          }
        }
        const obj = JSON.parse(candidate);
        const id = String(obj.id || '').trim();
        const tool = String(obj.tool || '').trim();
        if (!id || !tool) {
          if (_dbg()) console.log('[ChatCMD ToolBridge] parseToolCalls: skipped malformed block (missing id or tool)', { id, tool, reason: 'missing_id_or_tool' });
          return;
        }
        if (seen.has(id)) {
          if (_dbg()) console.log('[ChatCMD ToolBridge] parseToolCalls: skipped duplicate call in same message', { callId: id, tool, reason: 'duplicate_in_same_message' });
          return;
        }
        seen.add(id);
        calls.push({ id, tool, arguments: obj.arguments || {} });
      } catch (err) {
        if (_dbg()) console.log('[ChatCMD ToolBridge] parseToolCalls: skipped block due to JSON parse error', { error: String(err?.message || err), reason: 'json_parse_error' });
      }
    }

    // 1. Fenced blocks: ```chatcmd_tool_call ... ``` (with optional spaces/newlines)
    const fencedPattern = /`{3,}\s*chatcmd_tool_call\b[\s\S]*?\n([\s\S]*?)`{3,}/gi;
    let m;
    while ((m = fencedPattern.exec(text)) !== null) {
      tryAdd(m[1]);
    }

    // 2. Inline tags: [[CHATCMD_TOOL_CALL:{...}]]
    const inlinePattern = /\[\[CHATCMD_TOOL_CALL:(\{[\s\S]*?\})\]\]/gi;
    while ((m = inlinePattern.exec(text)) !== null) {
      tryAdd(m[1]);
    }

    // 3. Fallback: Any JSON code block with "tool" and "id".
    //    IMPORTANT: skip blocks tagged with chatcmd_* language identifiers other than
    //    chatcmd_tool_call (e.g. chatcmd_tool_result), to prevent result blocks from
    //    being mistakenly re-dispatched as new calls after reset().
    if (calls.length === 0) {
      const genericBlockPattern = /`{3,}([^\n`]*)\n([\s\S]*?)`{3,}/gi;
      while ((m = genericBlockPattern.exec(text)) !== null) {
        const tag = m[1].trim().toLowerCase();
        // Skip blocks that are explicitly tagged as non-call chatcmd protocol blocks.
        if (tag.startsWith('chatcmd_') && tag !== 'chatcmd_tool_call') {
          if (_dbg()) console.log('[ChatCMD ToolBridge] parseToolCalls: skipping block tagged', JSON.stringify(tag), { reason: 'tagged_non_call' });
          continue;
        }
        const candidate = m[2].trim();
        if (candidate.includes('"tool"') && candidate.includes('"id"')) {
          tryAdd(candidate);
        }
      }
    }

    return calls;
  }

  /**
   * Returns true if `text` contains at least one tool call that has not
   * yet been dispatched.
   */
  function hasPendingToolCalls(text) {
    const parsed = parseToolCalls(text);
    const pending = parsed.filter((c) => !executedCallIds.has(c.id));
    if (_dbg() && parsed.length > 0 && pending.length === 0) {
      for (const c of parsed) {
        console.log('[ChatCMD ToolBridge] hasPendingToolCalls: skipped already-executed call', { callId: c.id, tool: c.tool, reason: 'already_in_executedCallIds' });
      }
    }
    return pending.length > 0;
  }

  // ── Formatting ─────────────────────────────────────────────────────────

  /**
   * Build the tool-result block that will be injected into the ChatGPT
   * composer so ChatGPT can read the result and continue its task.
   *
   * errorInfo may be:
   *  - an object with { code, message } (normal backend RuntimeError)
   *  - an object with { message } (network-level error from background-io)
   *  - a plain string  (catch path in background.js, e.g. extension error)
   *  - undefined/null  (unknown failure)
   */
  function formatResult(callId, toolName, ok, result, errorInfo) {
    if (ok) {
      const content = typeof result === 'string'
        ? result
        : JSON.stringify(result, null, 2);
      return `\`\`\`chatcmd_tool_result\n{"id":"${callId}","tool":"${toolName}","ok":true,"content":${JSON.stringify(content)}}\n\`\`\``;
    }
    // Normalise errorInfo into a human-readable message.
    let msg;
    if (typeof errorInfo === 'string') {
      msg = errorInfo || 'Tool execution failed.';
    } else if (errorInfo && typeof errorInfo === 'object') {
      const code = errorInfo.code ? `[${errorInfo.code}] ` : '';
      msg = errorInfo.message ? `${code}${errorInfo.message}` : (code || 'Tool execution failed.');
    } else {
      msg = 'Tool execution failed.';
    }
    return `\`\`\`chatcmd_tool_result\n{"id":"${callId}","tool":"${toolName}","ok":false,"error":${JSON.stringify(msg)}}\n\`\`\``;
  }

  // ── Execution ──────────────────────────────────────────────────────────

  /**
   * Ask the background service worker to execute a single tool call via
   * POST /api/local/chatgpt/bridge/tools/call.
   *
   * Returns a promise that resolves to { ok, result?, error? }.
   */
  function executeToolCall(requestId, call) {
    if (_dbg()) console.log('[ChatCMD ToolBridge] executeToolCall:', { requestId, call });
    return new Promise((resolve, reject) => {
      globalThis.ChatCmdRuntime.sendMessage({
        type: 'chatcmd-chatgpt-tool-call',
        requestId,
        callId: call.id,
        tool: call.tool,
        arguments: call.arguments,
      }).then(resolve).catch(reject);
    });
  }

  /**
   * Execute all pending tool calls found in `text`, then compose a single
   * reply block containing all results and submit it to ChatGPT.
   *
   * `requestId` is the bridge request ID (authoritative identity source).
   * `submitFn(text)` fills the composer and submits (provided by content-chatgpt.js).
   *
   * Returns true if at least one tool call was dispatched.
   */
  async function runPendingCalls(requestId, text, submitFn) {
    const allParsed = parseToolCalls(text);
    const calls = allParsed.filter((c) => {
      if (executedCallIds.has(c.id)) {
        if (_dbg()) console.log('[ChatCMD ToolBridge] runPendingCalls: skipping already-executed call', c.id, 'tool=', c.tool, 'requestId=', requestId, 'reason=already_in_executedCallIds');
        return false;
      }
      return true;
    });
    if (!calls.length) {
      if (_dbg()) {
        if (allParsed.length > 0) {
          console.log('[ChatCMD ToolBridge] runPendingCalls: all tool calls skipped', { requestId, totalParsed: allParsed.length, reason: 'all_calls_already_executed' });
        } else {
          console.log('[ChatCMD ToolBridge] runPendingCalls: no tool calls found in text', { requestId, reason: 'no_calls_parsed' });
        }
      }
      return false;
    }

    // Mark all as dispatched immediately (first-layer dedupe).
    if (_dbg()) console.log('[ChatCMD ToolBridge] runPendingCalls: dispatching', { requestId, callIds: calls.map((c) => `${c.id}(${c.tool})`), allParsedIds: allParsed.map((c) => c.id) });
    for (const c of calls) {
      executedCallIds.add(c.id);
    }

    // Execute all calls in parallel.
    const results = await Promise.allSettled(
      calls.map(async (call) => {
        if (inflight.has(call.id)) return inflight.get(call.id);
        const promise = executeToolCall(requestId, call);
        inflight.set(call.id, promise);
        try {
          return await promise;
        } finally {
          inflight.delete(call.id);
        }
      })
    );

    // Build the combined result block.
    const blocks = [];
    let anyError = false;
    let residentFooter = null;
    const errorTools = [];

    for (let i = 0; i < calls.length; i++) {
      const call = calls[i];
      const outcome = results[i];
      if (outcome.status === 'fulfilled') {
        const r = outcome.value;
        blocks.push(formatResult(call.id, call.tool, r?.ok === true, r?.result, r?.error));
        if (r?.ok !== true) {
          anyError = true;
          errorTools.push(call.tool);
        }
        // Use the last non-null residentFooter seen across all responses.
        if (r?.residentFooter) {
          residentFooter = r.residentFooter;
        }
      } else {
        blocks.push(formatResult(call.id, call.tool, false, null, { message: String(outcome.reason?.message || outcome.reason || 'Unknown error') }));
        anyError = true;
        errorTools.push(call.tool);
      }
    }

    // Compute next_id from the max numeric suffix seen in all call IDs in text.
    let nextId = null;
    try {
      const idPattern = /\bcall_(\d+)\b/g;
      let maxNum = -1;
      let m;
      while ((m = idPattern.exec(text)) !== null) {
        const n = parseInt(m[1], 10);
        if (n > maxNum) maxNum = n;
      }
      // Also check call IDs from results.
      for (const call of calls) {
        const dm = call.id.match(/^call_(\d+)$/);
        if (dm) {
          const n = parseInt(dm[1], 10);
          if (n > maxNum) maxNum = n;
        }
      }
      if (maxNum >= 0) {
        const digits = String(maxNum).length;
        nextId = 'call_' + String(maxNum + 1).padStart(digits, '0');
      }
    } catch { /* ignore */ }

    // Assemble reply: tool result blocks first, then [ChatCMD 提醒] footer.
    let reply = blocks.join('\n\n');

    const footerLines = [];
    if (residentFooter) {
      footerLines.push(residentFooter);
    }
    if (nextId) {
      footerLines.push(`next_id: ${nextId}`);
    }
    if (anyError && errorTools.length > 0) {
      const toolList = [...new Set(errorTools)];
      const hints = toolList.map((t) =>
        `如工具 ${t} 失敗，請用 skill_read(tier="core", tool="${t}") 取得說明` +
        (t ? `；若有 examples/${t}.md，請確認 tier 與 tool 參數用法` : '')
      );
      footerLines.push(hints.join('；'));
    }

    if (footerLines.length > 0) {
      reply += '\n\n[ChatCMD 提醒]\n' + footerLines.join('\n');
    }

    await submitFn(reply);
    return true;
  }


  function reset() {
    executedCallIds.clear();
    inflight.clear();
  }

  // ── Public API ─────────────────────────────────────────────────────────

  globalThis.ChatCmdToolBridge = Object.freeze({
    hasPendingToolCalls,
    runPendingCalls,
    reset,
    /** Diagnostic: returns the call IDs parseToolCalls finds in `text`. */
    parseToolCallIds: (text) => parseToolCalls(text).map((c) => `${c.id}(${c.tool})`),
    /** Diagnostic: live snapshot of executedCallIds (read-only). */
    get _executedCallIds() { return new Set(executedCallIds); },
  });
})();
