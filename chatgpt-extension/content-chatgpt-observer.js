// Request-scoped snapshots: event-driven reads, one in-flight write, retry until acknowledged.
(() => {
  const STORAGE_PREFIX = 'chatcmd-think:';
  function restore(requestId) {
    try { return JSON.parse(sessionStorage.getItem(STORAGE_PREFIX + requestId) || 'null'); }
    catch { return null; }
  }
  function matchesSubmitted(dom, targetText, expectedContent) {
    const normTarget = dom.normalize(targetText);
    const normExpected = dom.normalize(expectedContent);
    if (!normTarget || !normExpected) return false;
    if (normTarget === normExpected) return true;
    if (expectedContent.includes('chatcmd_tool_result')) {
      const stripped = dom.normalize(expectedContent.replace(/`{3,}(?:chatcmd_tool_result)?/g, ''));
      if (normTarget === stripped || normTarget.includes(stripped)) return true;
      const idMatches = [...expectedContent.matchAll(/"id"\s*:\s*"([^"]+)"/g)].map((m) => m[1]);
      if (idMatches.length > 0 && idMatches.some((id) => normTarget.includes(id))) return true;
    }
    return false;
  }
  function create(requestId, submittedContent, { resumed = false, user = null, current = () => true } = {}) {
    const dom = globalThis.ChatCmdTranscript;
    if (!dom || requestId.startsWith('subagent:')) return null;
    const prior = resumed ? restore(requestId) : null;
    const baseline = dom.latestUser()?.id;
    let userId = prior?.userId || user?.id || null;
    let userNode = user?.node || null;
    let conversationId = prior?.conversationId || (user ? dom.conversationId() : '');
    let messages = Array.isArray(prior?.messages) ? prior.messages : [];
    messages = messages.filter((message) => !dom.isNonAnswerUiText?.(message?.content));
    let revision = Number(prior?.revision) || Date.now();
    let acknowledged = 0;
    let complete = prior?.completed === true;
    let dirty = true;
    let stopped = false;
    let bound = resumed;
    let queued = false;
    let timer = null;
    let inFlight;
    let lastSend = 0;
    let lastScanSignature = '';
    let fallbackRescanUntil = Date.now() + 180_000;
    const clock = globalThis.ChatCmdCaptureClock;
    const later = (fn, ms) => clock ? clock.later(fn, ms) : setTimeout(fn, ms);
    const cancel = (id) => clock ? clock.cancel(id) : clearTimeout(id);
    const ids = new WeakMap();
    function checkpoint() {
      try { sessionStorage.setItem(STORAGE_PREFIX + requestId, JSON.stringify({ requestId, submittedContent,
        userId, conversationId, messages, revision, completed: complete })); } catch { /* SQLite remains authoritative. */ }
    }
    function scan() {
      if (stopped || !current()) return;
      const id = dom.conversationId();
      if (conversationId && id && id !== conversationId) {
        if (/^WEB:/i.test(conversationId) && userId && dom.latestUser()?.id === userId) { conversationId = id; checkpoint(); }
        else { stop(); return; }
      }
      if (!conversationId && id) conversationId = id;
      // Preserve the original submitted bubble on role-less ChatGPT layouts;
      // otherwise the generic text fallback may select the newer assistant
      // bubble and stop this request-scoped observer before it captures output.
      const user = dom.latestUser(userNode);
      if (!user) return;
      if (!userId) {
        if ((!resumed && user.id === baseline) || !matchesSubmitted(dom, user.text, submittedContent)) return;
        userId = user.id;
        userNode = user.node;
        checkpoint();
      }
      if (user.id !== userId) {
        if (user.node === userNode && matchesSubmitted(dom, user.text, submittedContent) && user.node.isConnected) {
          userId = user.id;
          checkpoint();
        } else {
          stop();
          return;
        }
      }
      const used = new Set();
      let changed = false;
      const parts = dom.readParts(user);
      const toolCallParts = parts.filter((part) => /```+\s*chatcmd_tool_call\b/i.test(part.content));
      const toolBlocks = dom.toolBlockDiagnostics?.(user) || 'visiblePre=unknown';
      const scanSignature = `${parts.map((part) => `${part.kind}:${part.content.length}:${/```+\s*chatcmd_tool_call\b/i.test(part.content) ? 1 : 0}`).join(',')}|${toolBlocks}`;
      if (scanSignature !== lastScanSignature) {
        lastScanSignature = scanSignature;
        fallbackRescanUntil = Date.now() + 180_000;
        globalThis.ChatCmdCaptureStatus?.report('transcript-scan',
          `parts=${parts.length}; answerParts=${parts.filter((part) => part.kind === 'answer').length}; `
          + `commentaryParts=${parts.filter((part) => part.kind === 'commentary').length}; `
          + `toolCallParts=${toolCallParts.length}; chars=${parts.reduce((total, part) => total + part.content.length, 0)}; ${toolBlocks}`);
      }
      if (!parts.some((part) => part.kind === 'answer')) {
        globalThis.ChatCmdCaptureStatus?.report('transcript-empty', dom.readDiagnostics?.(user, parts) || `parsedParts=${parts.length}`);
      }
      for (const [index, part] of parts.entries()) {
        let message = ids.get(part.node);
        if (!message) {
          message = messages.find((entry) => !used.has(entry.id) &&
            (entry.content === part.content || part.content.startsWith(entry.content) || entry.content.startsWith(part.content)));
          if (!message && part.messageId) message = messages.find((entry) => entry.id === `${part.messageId}:${index}`);
        }
        if (!message) {
          if (messages.length >= 128) break;
          message = { id: part.messageId ? `${part.messageId}:${index}` : `part-${revision}-${messages.length}`,
            kind: part.kind, content: '' };
          messages.push(message);
        }
        ids.set(part.node, message);
        used.add(message.id);
        if (message.content.startsWith(part.content) && message.content.length > part.content.length) continue;
        const budget = 100_000 - messages.reduce((total, entry) => total + (entry === message ? 0 : [...entry.content].length), 0);
        const content = [...part.content].slice(0, Math.max(0, budget)).join('');
        if (content && (message.content !== content || message.kind !== part.kind)) {
          message.content = content;
          message.kind = part.kind;
          changed = true;
        }
      }
      messages = messages.filter((message) => message.content);
      const withoutUiText = messages.filter((message) => !dom.isNonAnswerUiText?.(message.content));
      if (withoutUiText.length !== messages.length) {
        messages = withoutUiText;
        changed = true;
      }
      if (changed) { dirty = true; revision = Math.max(revision + 1, Date.now()); checkpoint(); }
    }
    async function flush(completed = false, scanFirst = true) {
      if (scanFirst) scan();
      // ChatGPT can signal completion just before the final rendered bubble is
      // attached. Keep this request-scoped observer alive briefly and rescan the
      // rendered DOM so a late final mutation is not persisted as an empty turn.
      const capturedAnswerChars = () => messages.filter((message) => message.kind === 'answer')
        .reduce((total, message) => total + (message.content?.length || 0), 0);
      // A short prefix can be the only visible fragment while ChatGPT is still
      // attaching the rest of a streamed answer or its tool-call block. Reconcile
      // short captures too; an answer's mere presence is not proof it is complete.
      if (completed && capturedAnswerChars() < 120) {
        const deadline = Date.now() + 5_000;
        while (!stopped && current() && Date.now() < deadline && capturedAnswerChars() < 120) {
          await new Promise((resolve) => later(resolve, 200));
          scan();
        }
        if (capturedAnswerChars() < 120) {
          const latest = dom.latestUser?.();
          globalThis.ChatCmdCaptureStatus?.report('transcript-empty-at-completion',
            `bound=${bound}; userBound=${Boolean(userId)}; userMatches=${Boolean(latest && matchesSubmitted(dom, latest.text, submittedContent))}; `
            + `answerChars=${capturedAnswerChars()}; `
            + `${latest ? (dom.readDiagnostics?.(latest) || 'diagnostics=unavailable') : 'latestUser=missing'}`);
        }
      }
      if (completed && !complete) {
        complete = true; dirty = true; revision = Math.max(revision + 1, Date.now()); checkpoint();
      }
      if (inFlight) {
        const ok = await inFlight;
        if (ok && dirty && !stopped) return flush(completed);
        return !dirty;
      }
      if (!bound || stopped || !current() || !conversationId || !userId) return false;
      if (!dirty) return true;
      const sentRevision = revision;
      const payload = { type: 'chatcmd-chatgpt-progress', stage: 'observation', requestId,
        conversationId, conversationUrl: location.href, userMessageId: userId, revision: sentRevision,
        messages: messages.map((message) => ({ ...message })), completed: complete };
      lastSend = Date.now();
      inFlight = globalThis.ChatCmdRuntime.sendMessage(payload).then((response) => {
        if (!response?.ok || !response.accepted) {
          globalThis.ChatCmdCaptureStatus?.report('error', response?.error || 'Snapshot was not acknowledged');
          return false;
        }
        globalThis.ChatCmdCaptureStatus?.report('synced');
        acknowledged = Math.max(acknowledged, sentRevision);
        dirty = revision > acknowledged;
        return true;
      }).catch((error) => { globalThis.ChatCmdCaptureStatus?.report('error', String(error?.message || error)); return false; });
      try { return await inFlight; } finally { inFlight = null; scheduleSend(); }
    }
    function scheduleSend() {
      if (!bound || !dirty || stopped || !current() || inFlight) return;
      if (timer !== null) cancel(timer);
      // Always retain the trailing update. The last mutation may fall inside the rate limit.
      timer = later(() => { timer = null; void flush(); }, Math.max(25, 500 - (Date.now() - lastSend)));
    }
    function schedule() {
      if (queued || stopped || !current()) return;
      queued = true;
      void Promise.resolve().then(() => {
        queued = false;
        if (stopped || !current()) return;
        scan();
        if (bound && dirty && !inFlight && Date.now() - lastSend >= 500) void flush(false, false);
        else scheduleSend();
      });
    }
    const observerOptions = { childList: true, characterData: true, subtree: true,
      attributes: true, attributeFilter: ['class', 'role', 'aria-label', 'data-testid', 'data-message-id',
        'data-message-author-role', 'data-turn-id', 'data-turn', 'data-interrupted', 'data-language',
        'data-tool-call-id', 'hidden', 'aria-hidden'] };
    const observedRoots = new WeakSet();
    let observedShadowRootCount = 0;
    let shadowDiscoveryTimer = null;
    const observer = typeof MutationObserver === 'function' ? new MutationObserver((records) => {
      let foundShadowRoot = false;
      for (const record of records) {
        for (const node of record.addedNodes || []) {
          if (node.nodeType === Node.ELEMENT_NODE || node.nodeType === Node.DOCUMENT_FRAGMENT_NODE) {
            foundShadowRoot = discoverShadowRoots(node) || foundShadowRoot;
          }
        }
      }
      if (foundShadowRoot) reportObservedShadowRoots();
      schedule();
    }) : null;
    function discoverShadowRoots(root) {
      if (!observer || !root?.querySelectorAll) return false;
      const elements = [];
      if (root instanceof Element) elements.push(root);
      elements.push(...root.querySelectorAll('*'));
      let found = false;
      for (const element of elements) {
        if (!element.shadowRoot || observedRoots.has(element.shadowRoot)) continue;
        observedRoots.add(element.shadowRoot);
        observedShadowRootCount += 1;
        observer.observe(element.shadowRoot, observerOptions);
        found = true;
        found = discoverShadowRoots(element.shadowRoot) || found;
      }
      return found;
    }
    function reportObservedShadowRoots() {
      globalThis.ChatCmdCaptureStatus?.report('capture-shadow-roots-observed', `count=${observedShadowRootCount}`);
    }
    function discoverShadowRootsPeriodically() {
      if (stopped || !observer) return;
      shadowDiscoveryTimer = later(() => {
        shadowDiscoveryTimer = null;
        if (stopped) return;
        if (discoverShadowRoots(document.documentElement)) reportObservedShadowRoots();
        // Some ChatGPT layouts update rendered text without a mutation visible to
        // this content script. Reconcile periodically for a bounded window after
        // the last observed change; ordinary mutation events remain the fast path.
        const capturedChars = messages.filter((message) => message.kind === 'answer')
          .reduce((total, message) => total + (message.content?.length || 0), 0);
        if (bound && capturedChars < 120 && Date.now() < fallbackRescanUntil) schedule();
        discoverShadowRootsPeriodically();
      }, 3000);
    }
    observer?.observe(document.documentElement, observerOptions);
    discoverShadowRoots(document.documentElement);
    reportObservedShadowRoots();
    discoverShadowRootsPeriodically();
    function onPageHide(event) { if (!event.persisted) stop(); }
    window.addEventListener('pagehide', onPageHide);
    function stop() {
      if (stopped) return;
      stopped = true; observer?.disconnect(); if (timer !== null) cancel(timer);
      if (shadowDiscoveryTimer !== null) cancel(shadowDiscoveryTimer);
      window.removeEventListener('pagehide', onPageHide);
    }
    function finish() {
      stop();
      if (!dirty) { try { sessionStorage.removeItem(STORAGE_PREFIX + requestId); } catch { /* optional checkpoint */ } }
    }
    return {
      scan, flush, stop, finish,
      bind() { bound = true; return flush(); },
      get answer() {
        return messages.filter((message) => message.kind === 'answer' && message.content?.trim())
          .map((message) => message.content.trim()).join('\n\n');
      },
      get userMessageId() { return userId; },
      get hasTurn() {
        return Boolean(userId) && messages.some((message) => message.kind === 'answer' && Boolean(message.content?.trim()));
      },
      get active() { return !stopped && current(); },
    };
  }
  globalThis.ChatCmdObserver = Object.freeze({ create, restore });
})();
