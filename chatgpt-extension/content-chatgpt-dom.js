(() => {
  function normalize(value) {
    return String(value || '').replace(/\s+/g, ' ').trim().toLowerCase();
  }

  function isVisible(element) {
    if (!(element instanceof Element)) return false;
    const rect = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
  }

  function findVisible(selectors) {
    return findVisibleWithin(document, selectors);
  }

  function findVisibleWithin(root, selectors) {
    for (const selector of selectors) {
      for (const element of root.querySelectorAll(selector)) if (isVisible(element)) return element;
    }
    return null;
  }

  function queryAllIncludingShadow(selector, root = document) {
    const found = [...root.querySelectorAll(selector)];
    for (const element of root.querySelectorAll('*')) {
      if (element.shadowRoot) found.push(...queryAllIncludingShadow(selector, element.shadowRoot));
    }
    return [...new Set(found)];
  }

  function appearsAfter(left, right) {
    if (!left || !right) return true;
    const relation = left.compareDocumentPosition(right);
    if (!(relation & Node.DOCUMENT_POSITION_DISCONNECTED)) {
      return Boolean(relation & Node.DOCUMENT_POSITION_FOLLOWING);
    }
    // compareDocumentPosition reports nodes in open Shadow DOM as disconnected.
    // Their rendered viewport positions still provide a useful chronological
    // check for an error bubble below the submitted message.
    const leftRect = left.getBoundingClientRect();
    const rightRect = right.getBoundingClientRect();
    return rightRect.top > leftRect.top
      || (rightRect.top === leftRect.top && rightRect.left >= leftRect.right);
  }

  function assistantNodes() {
    const isUiText = (node) => globalThis.ChatCmdTranscript?.isNonAnswerUiText?.(node?.innerText || node?.textContent || '') || false;
    const semantic = [...document.querySelectorAll('[data-message-author-role="assistant"]')]
      .filter((node) => isVisible(node) && !isUiText(node));
    if (semantic.length) return semantic;
    const user = globalThis.ChatCmdTranscript?.latestUser?.();
    return user ? globalThis.ChatCmdTranscript.readParts(user)
      .filter((part) => part.kind === 'answer' && isVisible(part.node) && !isUiText(part.node)).map((part) => part.node) : [];
  }

  function latestMessageText(role) {
    if (role === 'user') {
      const user = globalThis.ChatCmdTranscript?.latestUser?.();
      if (user?.content) return user.content;
    }
    const nodes = [...document.querySelectorAll(`[data-message-author-role="${role}"]`)].filter(isVisible);
    const latest = (role === 'assistant' ? assistantNodes() : nodes).at(-1);
    return latest?.innerText?.trim() || latest?.textContent?.trim() || '';
  }

  function findThreadError() {
    const latestUser = globalThis.ChatCmdTranscript?.latestUser?.()?.node
      || [...document.querySelectorAll('[data-message-author-role="user"]')].filter(isVisible).at(-1);
    const latestAssistant = assistantNodes().at(-1);
    const candidates = queryAllIncludingShadow([
      'button[data-testid="regenerate-thread-error-button"]',
      '[class*="text-token-text-error"]',
      '[class*="bg-token-surface-error"]',
      '[class*="border-token-surface-error"]',
    ].join(',')).filter(isVisible);
    return candidates.reverse().find((element) => {
      const afterLatestUser = appearsAfter(latestUser, element);
      if (!afterLatestUser) return false;
      if (!latestAssistant) return true;
      const assistantAfterError = appearsAfter(element, latestAssistant);
      return !assistantAfterError;
    }) || null;
  }

  function isTooLongText(text) {
    const s = normalize(text);
    return s.includes('你提交的訊息太長')
      || s.includes('訊息太長')
      || s.includes('message you submitted was too long')
      || s.includes('message is too long')
      || s.includes('message too long');
  }

  function findTooLongSubmissionError() {
    const candidateSelectors = [
      '[role="alert"]',
      '[data-testid*="thread-error" i]',
      '[data-testid*="error" i]',
      '[class*="text-token-text-error"]',
      '[class*="bg-token-surface-error"]',
      '[class*="border-token-surface-error"]',
      '[class*="text-red" i]',
      '[class*="error" i]',
    ].join(',');
    const candidates = queryAllIncludingShadow(candidateSelectors).filter((element) => {
      if (!isVisible(element)) return false;
      const text = element.innerText || element.textContent || '';
      return text.length < 500 && isTooLongText(text);
    });
    if (candidates.length) return candidates.at(-1);

    const fallbacks = [...document.querySelectorAll('div, p, span, section')].filter((element) => {
      if (!isVisible(element)) return false;
      const text = element.innerText || element.textContent || '';
      return text.length < 300 && isTooLongText(text);
    });
    return fallbacks.at(-1) || null;
  }

  function findStopButton() {
    const composers = [...document.querySelectorAll('form[data-type="unified-composer"]')].filter(isVisible);
    const root = composers.at(-1) || document;
    const direct = findVisibleWithin(root, [
      'button[data-testid="stop-button"]',
      'button[data-testid="stop-generating-button"]',
      'button[aria-label*="Stop" i]',
      'button[aria-label*="Dừng" i]',
      'button[aria-label*="Ngừng" i]',
    ]);
    if (direct) return direct;
    return [...root.querySelectorAll('button')].filter(isVisible).find((button) =>
      /^(stop|dừng|ngừng)(?:\s|$)/i.test(normalize(button.getAttribute('aria-label') || button.textContent))) || null;
  }

  function clickStopButton() {
    const button = findStopButton();
    if (button) button.click();
  }

  function findSendButton(composer = null) {
    const selectors = [
      'button[data-testid="send-button"]',
      'button[data-testid="composer-submit-button"]',
      'button[aria-label="Send prompt"]',
      'button[aria-label="Send message"]',
      'button[aria-label="Send"]',
      'button[aria-label*="傳送"],button[aria-label*="送出"],button[aria-label*="發送"]',
      'button[title*="傳送"],button[title*="送出"],button[title*="發送"]',
    ];
    const scope = composer?.closest('form,[data-type="unified-composer"],[data-testid*="composer" i]') || document;
    const exact = findVisibleWithin(scope, selectors);
    if (exact) return exact;
    const submit = [...scope.querySelectorAll('button[type="submit"]')].find(isVisible);
    if (submit) return submit;
    // Fail closed: an unlabeled control after the composer may be the add-tools (+)
    // button. Never guess which control submits the message based on DOM order.
    return null;
  }

  globalThis.ChatCmdConversationDom = Object.freeze({
    assistantNodes,
    clickStopButton,
    findSendButton,
    findStopButton,
    findThreadError,
    findTooLongSubmissionError,
    findVisible,
    isTooLongText,
    isVisible,
    latestMessageText,
    normalize,
  });
})();
