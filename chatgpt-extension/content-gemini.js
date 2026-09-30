(() => {
  'use strict';

  const CONTENT_CONTEXT = globalThis.ChatCmdRuntime.install('gemini');
  const REQUEST_TYPE = 'chatcmd-gemini-run';
  const RESULT_TYPE = 'chatcmd-gemini-result';

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  function visible(el) {
    if (!el) return false;
    const s = getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden' && el.getClientRects().length > 0;
  }

  function findComposer() {
    const selectors = [
      'rich-textarea div[contenteditable="true"]',
      'rich-textarea [contenteditable="true"]',
      'div[contenteditable="true"][role="textbox"]',
      'div[contenteditable="true"]',
      'textarea[aria-label*="prompt" i]',
      'textarea[placeholder*="prompt" i]',
      'textarea'
    ];
    for (const selector of selectors) {
      const nodes = [...document.querySelectorAll(selector)];
      const node = nodes.find(visible);
      if (node) return node;
    }
    return null;
  }

  async function setComposerText(composer, text) {
    composer.focus();
    if (composer instanceof HTMLTextAreaElement || composer instanceof HTMLInputElement) {
      const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(composer), 'value')?.set;
      setter?.call(composer, text);
      composer.dispatchEvent(new Event('input', { bubbles: true }));
      composer.dispatchEvent(new Event('change', { bubbles: true }));
      return;
    }

    try {
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(composer);
      selection.removeAllRanges();
      selection.addRange(range);
      const inserted = document.execCommand('insertText', false, text);
      if (inserted) {
        composer.dispatchEvent(new Event('input', { bubbles: true }));
        return;
      }
    } catch {}

    composer.innerText = text;
    composer.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text }));
    composer.dispatchEvent(new Event('change', { bubbles: true }));
  }

  function findSendButton() {
    const selectors = [
      'button[aria-label*="send" i]',
      'button[aria-label*="傳送" i]',
      'button[aria-label*="發送" i]',
      'button[aria-label*="提交" i]',
      'button[aria-label*="submit" i]',
      'button[mattooltip*="send" i]',
      'button[mattooltip*="傳送" i]',
      'button.send-button',
      'button[data-test-id*="send" i]',
      'button[data-testid*="send" i]'
    ];
    for (const selector of selectors) {
      const node = [...document.querySelectorAll(selector)].find(visible);
      if (node && !node.disabled && node.getAttribute('aria-disabled') !== 'true') return node;
    }
    return null;
  }

  function isGenerating() {
    const stopSelectors = [
      'button[aria-label*="stop" i]',
      'button[aria-label*="停止" i]',
      'button.stop-button',
      'button[mattooltip*="stop" i]',
      'button[mattooltip*="停止" i]',
      '.sparkle-container'
    ];
    return stopSelectors.some((selector) => {
      const node = document.querySelector(selector);
      return visible(node);
    });
  }

  function assistantElements() {
    const selectors = [
      'model-response message-content',
      'model-response',
      'message-content',
      '[data-message-author-role="model"]',
      '.model-response-text'
    ];
    for (const selector of selectors) {
      const nodes = [...document.querySelectorAll(selector)].filter(visible);
      if (nodes.length > 0) return nodes;
    }
    return [];
  }

  function latestAssistantText(elements) {
    if (!elements || elements.length === 0) return '';
    const last = elements[elements.length - 1];
    return (last.innerText || last.textContent || '').trim();
  }

  async function submitPrompt(composer) {
    const button = findSendButton();
    if (button) {
      button.click();
      return;
    }
    composer.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true }));
  }

  async function waitForAssistant(previousCount, requestId) {
    const deadline = Date.now() + 300000;
    let stableText = '';
    let stableSince = 0;
    while (Date.now() < deadline) {
      if (!globalThis.ChatCmdRuntime.current(CONTENT_CONTEXT)) {
        throw new Error('Gemini content context is no longer active.');
      }
      const elements = assistantElements();
      const generating = isGenerating();
      if (elements.length > previousCount) {
        const text = latestAssistantText(elements);
        if (text) {
          if (text === stableText) {
            if (!stableSince) stableSince = Date.now();
            if (!generating && Date.now() - stableSince >= 1500) {
              return text;
            }
          } else {
            stableText = text;
            stableSince = Date.now();
          }
        }
      }
      await sleep(300);
    }
    throw new Error(`Timed out waiting for Gemini response (${requestId}).`);
  }

  async function executeToolLoop(requestId, initialResult) {
    let currentResult = initialResult;
    while (globalThis.ChatCmdToolBridge?.hasPendingToolCalls(currentResult)) {
      const assistantCount = assistantElements().length;
      const submitToolResult = async (replyText) => {
        const composer = findComposer();
        if (!composer) throw new Error('Gemini composer was not found while submitting a tool result.');
        await setComposerText(composer, replyText);
        await submitPrompt(composer);
      };
      const dispatched = await globalThis.ChatCmdToolBridge.runPendingCalls(
        requestId,
        currentResult,
        submitToolResult,
      );
      if (!dispatched) break;
      currentResult = await waitForAssistant(assistantCount, requestId);
    }
    globalThis.ChatCmdToolBridge?.reset?.();
    return currentResult;
  }

  let activeRequest = null;

  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    if (!globalThis.ChatCmdRuntime.current(CONTENT_CONTEXT)) return false;

    if (message?.type === 'chatcmd-content-alive' && message.kind === 'gemini') {
      sendResponse({ ok: true, kind: 'gemini', captureReady: true });
      return false;
    }

    if (message?.type === 'chatcmd-gemini-ready') {
      const composer = findComposer();
      const generating = isGenerating();
      sendResponse({ ok: true, ready: Boolean(composer && !generating && !activeRequest) });
      return false;
    }

    if (message?.type !== REQUEST_TYPE) return false;
    if (activeRequest) {
      sendResponse({ ok: false, error: 'The Gemini tab is already processing a request.' });
      return false;
    }

    activeRequest = { id: message.requestId };
    sendResponse({ ok: true });

    void (async () => {
      try {
        let composer = findComposer();
        if (!composer) {
          for (let i = 0; i < 15; i += 1) {
            await sleep(400);
            composer = findComposer();
            if (composer) break;
          }
        }
        if (!composer) throw new Error('Gemini composer was not found.');

        const beforeCount = assistantElements().length;
        await setComposerText(composer, message.submittedContent || '');
        await sleep(200);
        await submitPrompt(composer);

        const initialAssistantContent = await waitForAssistant(beforeCount, message.requestId);
        const assistantContent = await executeToolLoop(message.requestId, initialAssistantContent);

        await chrome.runtime.sendMessage({
          type: RESULT_TYPE,
          requestId: message.requestId,
          conversationUrl: window.location.href,
          conversationId: conversationIdFromCurrentUrl() || ('GEMINI:' + window.location.pathname).slice(0, 500),
          assistantContent,
        });
      } catch (error) {
        await chrome.runtime.sendMessage({
          type: RESULT_TYPE,
          requestId: message.requestId,
          error: error instanceof Error ? error.message : String(error),
        });
      } finally {
        activeRequest = null;
      }
    })();

    return false;
  });

  // --- Proactive Discovery & Native Turn Capture ---

  const USER_SELECTORS = [
    'user-query message-content',
    'user-query .query-text',
    'user-query',
    '[data-message-author-role="user"]',
    '.user-query-container',
    '.user-query'
  ];

  function findUserElements() {
    for (const selector of USER_SELECTORS) {
      const nodes = [...document.querySelectorAll(selector)].filter(visible);
      if (nodes.length > 0) return nodes;
    }
    return [];
  }

  function fingerprint(text) {
    let hash = 2166136261;
    for (let i = 0; i < text.length; i++) hash = Math.imul(hash ^ text.charCodeAt(i), 16777619);
    return (hash >>> 0).toString(16);
  }

  function conversationIdFromCurrentUrl() {
    try {
      const match = location.pathname.match(/(?:^|\/)app\/([^/?#]+)/);
      return match ? decodeURIComponent(match[1]) : '';
    } catch { return ''; }
  }

  function latestUserQuery() {
    const nodes = findUserElements();
    if (!nodes.length) return null;
    const last = nodes[nodes.length - 1];
    const text = (last.innerText || last.textContent || '').trim();
    if (!text) return null;
    const id = last.getAttribute('data-message-id')
      || last.getAttribute('id')
      || ('gemini-user:' + nodes.length + ':' + fingerprint(text));
    return { node: last, id, content: text };
  }

  let lastDiscoveredId = null;
  let lastDiscoveredUrl = '';
  let pendingTurn = false;
  const capturedTurns = new Set();

  function observeAssistantResponse(requestId, convId, convUrl, userMessageId) {
    let stableText = '';
    let stableSince = 0;
    const maxWait = Date.now() + 180000;

    const timer = setInterval(() => {
      if (!globalThis.ChatCmdRuntime.current(CONTENT_CONTEXT) || conversationIdFromCurrentUrl() !== convId) {
        clearInterval(timer);
        return;
      }
      const currentElements = assistantElements();
      const generating = isGenerating();
      if (currentElements.length > 0) {
        const text = latestAssistantText(currentElements);
        if (text) {
          if (text === stableText) {
            if (!stableSince) stableSince = Date.now();
            if (!generating && Date.now() - stableSince >= 1500) {
              clearInterval(timer);
              void globalThis.ChatCmdRuntime.sendMessage({
                type: 'chatcmd-chatgpt-progress',
                requestId,
                stage: 'observation',
                conversationId: convId,
                conversationUrl: convUrl,
                userMessageId,
                revision: Date.now(),
                completed: true,
                messages: [{
                  id: 'gemini-ans:' + fingerprint(text),
                  kind: 'answer',
                  content: text,
                }],
              }).catch(() => {});
            }
          } else {
            stableText = text;
            stableSince = Date.now();
          }
        }
      }
      if (Date.now() > maxWait) clearInterval(timer);
    }, 500);
  }

  async function checkDiscoveryAndTurns() {
    if (!globalThis.ChatCmdRuntime.current(CONTENT_CONTEXT)) return;

    const convId = conversationIdFromCurrentUrl();
    const currentUrl = location.href;

    // 1. Report conversation identity change (A -> B switch, new chat, reload)
    if (convId && (convId !== lastDiscoveredId || currentUrl !== lastDiscoveredUrl)) {
      lastDiscoveredId = convId;
      lastDiscoveredUrl = currentUrl;
      void globalThis.ChatCmdRuntime.sendMessage({
        type: 'chatcmd-gemini-discovery',
        conversationId: convId,
        conversationUrl: currentUrl,
      }).catch(() => {});
    }

    // 2. Check for native user turn
    if (!convId || pendingTurn) return;
    const user = latestUserQuery();
    if (!user || !user.content.trim()) return;

    const turnKey = `${convId}\0${user.id}`;
    if (capturedTurns.has(turnKey)) return;

    if (activeRequest) return;

    pendingTurn = true;
    try {
      const response = await globalThis.ChatCmdRuntime.sendMessage({
        type: 'chatcmd-chatgpt-native-turn',
        conversationId: convId,
        conversationUrl: currentUrl,
        userMessageId: user.id,
        content: user.content,
      });
      if (response?.ok) {
        capturedTurns.add(turnKey);
        if (capturedTurns.size > 256) {
          capturedTurns.delete(capturedTurns.values().next().value);
        }
        if (response.request?.id && !response.request.hasFinalResponse) {
          observeAssistantResponse(response.request.id, convId, currentUrl, user.id);
        }
      }
    } catch {}
    finally {
      pendingTurn = false;
    }
  }

  let scheduled = false;
  function scheduleCheck() {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      void checkDiscoveryAndTurns();
    }, 300);
  }

  const observer = new MutationObserver(() => scheduleCheck());
  if (document.body) {
    observer.observe(document.body, { subtree: true, childList: true, characterData: true });
  } else {
    document.addEventListener('DOMContentLoaded', () => {
      if (document.body) observer.observe(document.body, { subtree: true, childList: true, characterData: true });
    });
  }

  window.addEventListener('popstate', () => scheduleCheck());
  try {
    const origPushState = history.pushState;
    history.pushState = function(...args) {
      const result = origPushState.apply(this, args);
      scheduleCheck();
      return result;
    };
    const origReplaceState = history.replaceState;
    history.replaceState = function(...args) {
      const result = origReplaceState.apply(this, args);
      scheduleCheck();
      return result;
    };
  } catch {}

  const pollTimer = setInterval(() => {
    if (!globalThis.ChatCmdRuntime.current(CONTENT_CONTEXT)) {
      clearInterval(pollTimer);
      return;
    }
    void checkDiscoveryAndTurns();
  }, 3000);

  setTimeout(() => void checkDiscoveryAndTurns(), 100);
})();
