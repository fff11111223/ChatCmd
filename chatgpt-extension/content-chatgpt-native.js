// Native ChatGPT sends and ChatCMD sends share one request-scoped recorder.
// Enrollment observes public messages only; it never presses Send or calls MCP.
(() => {
  globalThis.ChatCmdNativeCapture?.stop();
  const controller = globalThis.ChatCmdController;
  const dom = globalThis.ChatCmdTranscript;
  const clock = globalThis.ChatCmdCaptureClock;
  const later = (fn, ms, options) => clock ? clock.later(fn, ms, options) : setTimeout(fn, ms);
  const cancel = (id) => clock ? clock.cancel(id) : clearTimeout(id);
  const USER_SELECTOR = '[data-message-author-role="user"],[data-turn="user"]';
  const NATIVE_FALLBACK_POLL_MS = 4_000;
  const seen = new Set();
  let stopped = false;
  let pending = false;
  let retryAt = 0;
  let queued = false;
  let pollTimer;
  let retryTimer;
  let lastDiagnostic = '';
  function report(state, detail = '') {
    if (state !== 'controller-not-ready' && controller?.current && !controller.current()) return;
    document.documentElement.dataset.chatcmdCaptureState = state;
    document.documentElement.dataset.chatcmdCaptureError = detail;
    const diagnostic = `${state}:${detail}`;
    if (diagnostic === lastDiagnostic) return;
    lastDiagnostic = diagnostic;
    if (detail) console.warn('[ChatCMD capture]', detail);
    void globalThis.ChatCmdRuntime.sendMessage({ type: 'chatcmd-capture-diagnostic', state, detail }).catch(() => {});
  }
  globalThis.ChatCmdCaptureStatus = Object.freeze({ report });
  const conversationId = () => dom?.conversationId?.() || '';
  const keyFor = (user) => user && conversationId() ? `${conversationId()}\0${user.id}` : '';
  function remember(key) { seen.add(key); if (seen.size > 128) seen.delete(seen.values().next().value); }
  function retry(ms) {
    cancel(retryTimer);
    retryTimer = later(() => void tick(), ms);
  }
  async function tick() {
    if (stopped) return;
    if (!controller?.current?.()) { report('controller-not-ready'); return; }
    if (!dom?.latestUser || !dom?.conversationId) { report('capture-dependencies-missing', 'transcript adapter unavailable'); return; }
    let user;
    let key;
    try { user = dom.latestUser(); key = keyFor(user); }
    catch (error) { report('transcript-read-error', String(error?.message || error)); return; }
    if (!key || !user.content.trim()) {
      const scan = dom.userScanDiagnostics?.() || 'userScan=unavailable';
      report('waiting-for-user-message', `conversation=${Boolean(conversationId())}; userMessage=${Boolean(user)}; contentLength=${user?.content?.length || 0}; ${scan}`);
      return;
    }
    report('user-message-detected', `source=${user.source || 'semantic'}; contentLength=${user.content.length}`);
    if (globalThis.ChatCmdCompact?.busy || globalThis.ChatCmdCompactProtocol?.parse(user.content)) { report('turn-ignored', 'compact protocol owns the turn'); return; }
    const hasPendingTools = () => {
      const latestAssistant = dom.latestMessageText?.('assistant') || '';
      return Boolean(globalThis.ChatCmdToolBridge?.hasPendingToolCalls?.(latestAssistant));
    };
    if (user.content.trim().startsWith('```chatcmd_tool_result') && !hasPendingTools()) { remember(key); report('turn-ignored', 'tool result message'); return; }
    if (controller.active) {
      if (controller.active.observer?.userMessageId === user.id && controller.active.observer.active) remember(key);
      else if (controller.active.observer?.userMessageId) retry(500);
      return;
    }
    if (seen.has(key) || pending || Date.now() < retryAt) return;
    pending = true;
    try {
      report('capture-request-sending');
      const response = await globalThis.ChatCmdRuntime.sendMessage({ type: 'chatcmd-chatgpt-native-turn',
        conversationId: conversationId(), conversationUrl: location.href,
        userMessageId: user.id, content: user.content });
      if (stopped || !controller.current() || keyFor(dom.latestUser()) !== key || controller.active) return;
      if (response?.ok && response.ignored) { remember(key); report('capture-request-ignored'); return; }
      if (!response?.ok || !response.request) throw new Error(response?.error || 'ChatCMD did not acknowledge the browser turn.');
      report('capture-request-acknowledged', `status=${response.request.status || 'unknown'}`);
      remember(key);
      if (response.request.status === 'completed' && response.request.hasFinalResponse) return;
      void controller.adopt(response.request, user).then(() => {
        if (stopped || !controller.current()) return;
        report('recording');
        if (keyFor(dom.latestUser()) === key && document.documentElement.dataset.chatcmdCaptureState === 'error') {
          seen.delete(key); retryAt = Date.now() + 5000; retry(5000);
        }
        schedule();
      }).catch((error) => report('capture-adoption-error', String(error?.message || error)));
    } catch (error) {
      retryAt = Date.now() + 5000;
      report('error', String(error?.message || error));
      if (!stopped) retry(5000);
    } finally { pending = false; }
  }
  function schedule() {
    if (queued || stopped) return;
    queued = true;
    void Promise.resolve().then(() => { queued = false; if (!stopped) void tick(); });
  }
  function mutationAffectsUser(records) {
    for (const record of records) {
      const target = record.target?.nodeType === Node.TEXT_NODE ? record.target.parentElement : record.target;
      if (target?.closest?.(USER_SELECTOR)) return true;
      for (const added of record.addedNodes || []) {
        if (!(added instanceof Element)) continue;
        if (added.matches(USER_SELECTOR) || added.querySelector(USER_SELECTOR)) return true;
      }
    }
    return false;
  }
  const observer = new MutationObserver((records) => { if (mutationAffectsUser(records)) schedule(); });
  observer.observe(document.body, { subtree: true, childList: true, characterData: true });
  function poll() {
    if (stopped) return;
    void tick();
    // Idle polling is a low-frequency local fallback, never a perpetual service-worker keepalive.
    pollTimer = later(poll, NATIVE_FALLBACK_POLL_MS, { background: false });
  }
  function stop() {
    if (stopped) return;
    stopped = true; observer.disconnect(); cancel(pollTimer); cancel(retryTimer);
    document.removeEventListener('visibilitychange', schedule);
    window.removeEventListener('pageshow', schedule);
  }
  document.addEventListener('visibilitychange', schedule);
  window.addEventListener('pageshow', schedule);
  pollTimer = later(poll, NATIVE_FALLBACK_POLL_MS, { background: false });
  if (typeof chrome !== 'undefined' && chrome?.runtime?.onMessage?.addListener) {
    chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
      if (message?.type === 'chatcmd-trigger-native-capture') {
        (async () => {
          const deadline = Date.now() + 10_000;
          let foundUser = null;
          while (Date.now() < deadline) {
            try {
              foundUser = dom.latestUser?.();
              if (foundUser?.content?.trim()) break;
            } catch { /* wait for hydration */ }
            await (clock ? clock.sleep(250) : new Promise((r) => setTimeout(r, 250)));
          }
          if (!foundUser?.content?.trim()) {
            sendResponse({ ok: false, error: 'No user message found within timeout' });
            return;
          }
          const userKey = keyFor(foundUser);
          if (userKey) seen.delete(userKey);
          retryAt = 0;
          pending = false;
          await tick();
          sendResponse({ ok: true, conversationId: conversationId(), userMessageId: foundUser.id });
        })().catch((err) => sendResponse({ ok: false, error: String(err?.message || err) }));
        return true;
      }
      return false;
    });
  }
  globalThis.ChatCmdNativeCapture = Object.freeze({ stop, tick, schedule });
  report('capture-script-ready', `controller=${Boolean(controller)}; transcript=${Boolean(dom)}; conversation=${Boolean(conversationId())}`);
  Promise.resolve(globalThis.ChatCmdResumeReady).then(() => void tick());
})();
