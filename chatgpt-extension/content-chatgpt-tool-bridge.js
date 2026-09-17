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
        const obj = JSON.parse(raw.trim());
        const id = String(obj.id || '').trim();
        const tool = String(obj.tool || '').trim();
        if (!id || !tool || seen.has(id)) return;
        seen.add(id);
        calls.push({ id, tool, arguments: obj.arguments || {} });
      } catch { /* malformed JSON — skip */ }
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

    // 3. Fallback: Any JSON code block with "tool" and "id"
    if (calls.length === 0) {
      const genericBlockPattern = /`{3,}(?:json)?\s*\n([\s\S]*?)`{3,}/gi;
      while ((m = genericBlockPattern.exec(text)) !== null) {
        const candidate = m[1].trim();
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
    return parseToolCalls(text).some((c) => !executedCallIds.has(c.id));
  }

  // ── Formatting ─────────────────────────────────────────────────────────

  /**
   * Build the tool-result block that will be injected into the ChatGPT
   * composer so ChatGPT can read the result and continue its task.
   */
  function formatResult(callId, toolName, ok, result, errorInfo) {
    if (ok) {
      const content = typeof result === 'string'
        ? result
        : JSON.stringify(result, null, 2);
      return `\`\`\`chatcmd_tool_result\n{"id":"${callId}","tool":"${toolName}","ok":true,"content":${JSON.stringify(content)}}\n\`\`\``;
    }
    const msg = errorInfo?.message || 'Tool execution failed.';
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
    console.log('[ChatCMD ToolBridge] executeToolCall:', {
      requestId,
      call,
    });
    
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
    const calls = parseToolCalls(text).filter((c) => !executedCallIds.has(c.id));
    if (!calls.length) return false;

    // Mark all as dispatched immediately (first-layer dedupe).
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
    for (let i = 0; i < calls.length; i++) {
      const call = calls[i];
      const outcome = results[i];
      if (outcome.status === 'fulfilled') {
        const r = outcome.value;
        blocks.push(formatResult(call.id, call.tool, r?.ok === true, r?.result, r?.error));
      } else {
        blocks.push(formatResult(call.id, call.tool, false, null, { message: String(outcome.reason?.message || outcome.reason || 'Unknown error') }));
      }
    }

    const reply = blocks.join('\n\n');
    await submitFn(reply);
    return true;
  }

  // ── Public API ─────────────────────────────────────────────────────────

  globalThis.ChatCmdToolBridge = Object.freeze({
    hasPendingToolCalls,
    runPendingCalls,
  });
})();
