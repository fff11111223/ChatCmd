// Read only ChatGPT's public rendered transcript. Never inspect private model/reasoning state.
(() => {
  const USERS = '[data-message-author-role="user" i],[data-turn="user" i],[data-testid^="user-message"]';
  const USER_TEXT_FALLBACK = '.text-size-chat.whitespace-pre-wrap';
  const ASSISTANT_TEXT_FALLBACK = '.text-size-chat.whitespace-pre-wrap';
  const TURNS = '[data-testid^="conversation-turn"], [data-turn="assistant"], [data-message-author-role="assistant"], .markdown, '
    + ASSISTANT_TEXT_FALLBACK;
  const EXCLUDED = 'button,svg,script,style,.sr-only,.visually-hidden,[hidden],[aria-hidden="true"],[role="alert"],'
    + '[data-chatcmd-ui],.chatcmd-return-button,.chatcmd-approval-panel,.clf-stream,.clf-stage,'
    + 'input,textarea,[contenteditable="true"],[data-testid*="composer" i],'
    + '[data-testid*="tool-call"],[data-tool-call-id],span[class*="tool-message"],'
    + '[aria-label="Open tool call list" i]';
  const normalize = (text) => String(text || '').replace(/\s+/g, ' ').trim();
  function isFooterDisclaimer(text) {
    const value = normalize(text).toLowerCase().replace(/[.!。！]+$/g, '').trim();
    return /^(?:chatgpt\s+)?(?:可能會出錯|可能会出错)[。.!，,]?\s*(?:請查證重要資訊|请查证重要信息)$/.test(value)
      || /^chatgpt\s+(?:can make mistakes|may make mistakes)[.!]?\s*(?:check important info|please check important information)$/i.test(value)
      || /^chatgpt\s+can make mistakes\.\s+please check important info$/i.test(value);
  }
  function isActivityStatus(text) {
    const value = normalize(text).toLowerCase().replace(/[.!。！]+$/g, '').trim();
    return /^(?:已搜尋|搜尋了)\s*\d+\s*個網站$/.test(value)
      || /^(?:searched|searching)\s+\d+\s+sites?$/.test(value)
      || /^\d+\s+sites?\s+(?:searched|visited)$/.test(value);
  }
  function isNonAnswerUiText(text) { return isFooterDisclaimer(text) || isActivityStatus(text); }
  function queryAll(selector, root = document) {
    const found = [...root.querySelectorAll(selector)];
    for (const element of [...root.querySelectorAll('*')]) {
      if (element.shadowRoot) found.push(...queryAll(selector, element.shadowRoot));
    }
    return [...new Set(found)];
  }
  const conversationId = () => {
    const value = location.pathname.match(/^\/(?:g\/[^/]+\/)?c\/([^/?#]+)(?:\/|$)/)?.[1] || '';
    try { return decodeURIComponent(value); } catch { return value; }
  };
  function plainText(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent || '';
    if (!(node instanceof Element) || hidden(node)) return '';
    const text = [...node.childNodes].map(plainText).join('');
    return /^(P|DIV|SECTION|LI|BR|PRE)$/.test(node.tagName) ? '\n' + text + '\n' : text;
  }
  function userText(node) {
    const body = node.matches('[data-message-author-role="user"]') ? node
      : node.querySelector('[data-message-author-role="user"]') || node;
    const parts = queryAll('.whitespace-pre-wrap', body)
      .filter((part) => !part.parentElement?.closest('.whitespace-pre-wrap'));
    return (parts.length ? parts.map(plainText).join('\n') : plainText(body)).trim();
  }
  function fingerprint(text) {
    let hash = 2166136261;
    for (const char of text) hash = Math.imul(hash ^ char.codePointAt(0), 16777619);
    return (hash >>> 0).toString(16);
  }
  function latestUser() {
    let users = queryAll(USERS)
      .filter((node) => !node.parentElement?.closest(USERS));
    const semantic = users.length > 0;
    if (!semantic) users = queryAll(USER_TEXT_FALLBACK)
      .filter((node) => !node.closest('form,[contenteditable="true"],[data-testid*="composer"]') && !hidden(node));
    const node = users.at(-1);
    if (!node) return null;
    const content = userText(node);
    const id = node.getAttribute('data-message-id') || node.querySelector('[data-message-id]')?.getAttribute('data-message-id')
      || 'dom-user:' + (node.getAttribute('data-turn-id') || node.getAttribute('data-testid') || users.length - 1) + ':' + fingerprint(content);
    return { node, id, text: normalize(content), content, source: semantic ? 'semantic' : 'text-size-chat' };
  }
  function userScanDiagnostics() {
    const elements = queryAll('*');
    const matching = (selector) => elements.filter((node) => node.matches(selector));
    const roleUsers = matching('[data-message-author-role="user" i]').length;
    const turnUsers = matching('[data-turn="user" i]').length;
    const testIdUsers = matching('[data-testid^="user-message"]').length;
    const fallbackUsers = matching(USER_TEXT_FALLBACK).filter((node) => !node.closest('form,[contenteditable="true"],[data-testid*="composer"]') && !hidden(node)).length;
    const conversationTurns = matching('[data-testid^="conversation-turn"]');
    const rolelessTurns = conversationTurns.filter((node) => !node.matches(USERS)
      && !node.querySelector(USERS) && !node.querySelector('[data-message-author-role]')).length;
    const shadowRoots = elements.filter((node) => node.shadowRoot).length;
    const semantic = ['main', 'article', 'section', 'form'].map((tag) => `${tag}=${matching(tag).length}`).join(',');
    const roles = [...new Set(matching('[role]').map((node) => node.getAttribute('role')).filter(Boolean))].slice(0, 12).join(',') || 'none';
    const testIds = [...new Set(matching('[data-testid]').map((node) => node.getAttribute('data-testid')).filter(Boolean))].slice(0, 18).join(',') || 'none';
    const dataNames = [...new Set(elements.flatMap((node) => [...node.attributes].map((attr) => attr.name))
      .filter((name) => name.startsWith('data-')))].slice(0, 18).join(',') || 'none';
    const textContainers = elements.map((node) => ({
      tag: node.tagName.toLowerCase(), classes: [...node.classList].slice(0, 3).join('.'),
      length: [...node.childNodes].filter((child) => child.nodeType === Node.TEXT_NODE)
        .reduce((sum, child) => sum + normalize(child.textContent).length, 0),
    })).filter((item) => item.length >= 20).sort((a, b) => b.length - a.length).slice(0, 8)
      .map((item) => `${item.tag}${item.classes ? `.${item.classes}` : ''}:${item.length}`).join(',') || 'none';
    const frames = [...document.querySelectorAll('iframe')].map((frame) => {
      try { const url = new URL(frame.src, location.href); return `${url.origin}${url.pathname}`; }
      catch { return 'unresolved'; }
    }).slice(0, 4).join(',') || 'none';
    return `roleUsers=${roleUsers}; turnUsers=${turnUsers}; testIdUsers=${testIdUsers}; fallbackUsers=${fallbackUsers}; conversationTurns=${conversationTurns.length}; rolelessTurns=${rolelessTurns}; shadowRoots=${shadowRoots}; readyState=${document.readyState}; bodyTextLength=${normalize(document.body?.innerText).length}; semantic=${semantic}; roles=${roles}; testIds=${testIds}; dataAttrs=${dataNames}; textNodes=${textContainers}; iframes=${frames}`;
  }
  function hidden(node) {
    for (let current = node; current && current !== document.body; current = current.parentElement) {
      if (current.matches?.(EXCLUDED)) return true;
      const style = getComputedStyle(current);
      if (style.display === 'none' || style.visibility === 'hidden') return true;
    }
    return false;
  }
  function markdown(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent || '';
    if (!(node instanceof Element) || hidden(node)) return '';
    const tag = node.tagName.toLowerCase();
    if (tag === 'pre') {
      const code = node.querySelector('code');
      const langMatch = code?.className?.match(/language-([^\s]+)/);
      const lang = langMatch ? langMatch[1] : '';
      const text = (code ? (code.textContent || '') : plainText(node)).trimEnd();
      if (!text.trim()) return '';
      const fence = '`'.repeat(Math.max(3, ...[...text.matchAll(/`+/g)].map((match) => match[0].length + 1)));
      return `\n\n${fence}${lang}\n${text}\n${fence}\n\n`;
    }
    const text = [...node.childNodes].map(markdown).join('');
    if (tag === 'br') return '\n';
    if (tag === 'code') return `\`${text}\``;
    if (tag === 'strong' || tag === 'b') return `**${text}**`;
    if (tag === 'em' || tag === 'i') return `*${text}*`;
    if (/^h[1-6]$/.test(tag)) return `\n\n${'#'.repeat(Number(tag[1]))} ${text}\n\n`;
    if (tag === 'li') return `\n- ${text.trim()}`;
    if (tag === 'blockquote') return `\n\n${text.trim().split('\n').map((line) => `> ${line}`).join('\n')}\n\n`;
    if (tag === 'a') {
      const href = node.getAttribute('href') || '';
      return /^https?:\/\//i.test(href) ? `[${text}](${href.replace(/[()\s]/g, encodeURIComponent)})` : text;
    }
    if (['p', 'div', 'section', 'ul', 'ol', 'table', 'tr'].includes(tag)) return `\n${text}\n`;
    if (tag === 'td' || tag === 'th') return `${text} | `;
    return text;
  }
  function protocolToolCallParts(roots) {
    const parts = [];
    const seen = new Set();
    for (const root of roots) {
      for (const pre of queryAll('pre', root)) {
        const code = pre.querySelector('code');
        const classNames = `${pre.className || ''} ${code?.className || ''}`;
        const language = classNames.match(/\blanguage-([\w-]+)/i)?.[1]?.toLowerCase()
          || pre.getAttribute('data-language')?.toLowerCase();
        if (language !== 'chatcmd_tool_call' || !code || seen.has(code)) continue;
        let visible = true;
        for (let current = pre; current && current !== document.body; current = current.parentElement) {
          const style = getComputedStyle(current);
          if (current.hidden || current.getAttribute('aria-hidden') === 'true'
              || style.display === 'none' || style.visibility === 'hidden') {
            visible = false;
            break;
          }
        }
        if (!visible) continue;
        const text = (code.textContent || '').trimEnd();
        if (!text.trim()) continue;
        seen.add(code);
        const fence = '`'.repeat(Math.max(3, ...[...text.matchAll(/`+/g)].map((match) => match[0].length + 1)));
        parts.push({ node: pre, kind: 'answer', content: `${fence}chatcmd_tool_call\n${text}\n${fence}`,
          messageId: pre.closest('[data-message-id]')?.getAttribute('data-message-id') || '' });
      }
    }
    return parts;
  }
  function readParts(user) {
    if (!user?.node.isConnected) return [];
    const roots = queryAll(`${TURNS},[data-message-author-role="assistant"]`)
      .filter((node) => {
        const relation = node.compareDocumentPosition(user.node);
        return !(relation & Node.DOCUMENT_POSITION_DISCONNECTED)
          && Boolean(relation & Node.DOCUMENT_POSITION_PRECEDING);
      })
      .filter((node) => !node.matches('[data-turn="user"]') && !node.querySelector(USERS));
    const candidates = new Set();
    for (const root of roots) {
      const blocks = queryAll('.markdown,[data-interrupted]', root);
      if (root.matches('.markdown')) blocks.unshift(root);
      if (!blocks.length && (root.matches(ASSISTANT_TEXT_FALLBACK)
          || root.matches('[data-message-author-role="assistant"]'))) blocks.push(root);
      for (const block of blocks) {
        if (hidden(block)) continue;
        // An expanded tool-result body is not authored assistant prose.
        const tool = block.closest('[data-testid*="tool-call"],[data-tool-call-id],span[class*="tool-message"]');
        if (tool) continue;
        if (block.matches('[data-interrupted]') && block.querySelector('.markdown')) continue;
        if (block.matches('.markdown') && block.parentElement?.closest('.markdown')) continue;
        if (block.matches('[data-interrupted]') && block.parentElement?.closest('[data-interrupted]')) continue;
        candidates.add(block);
      }
    }
    const parts = [...candidates].sort((a, b) => a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1)
      .map((node) => ({ node, kind: node.closest('[data-interrupted]:not([data-interrupted="false"])') ? 'commentary' : 'answer',
        content: markdown(node).replace(/\n{3,}/g, '\n\n').trim(),
        messageId: node.closest('[data-message-id]')?.getAttribute('data-message-id') || '' }))
      .filter((part) => part.content && !isNonAnswerUiText(part.content));
    const protocolParts = protocolToolCallParts(roots);
    const combined = [...parts, ...protocolParts].filter((part, index, all) =>
      all.findIndex((candidate) => candidate.content === part.content) === index)
      .sort((a, b) => a.node.compareDocumentPosition(b.node) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1);
    if (combined.some((part) => part.kind === 'answer')) return combined;
    return rolelessAnswerParts(user);
  }
  function rolelessAnswerParts(user) {
    if (!user?.node?.isConnected) return [];
    const region = user.node.closest('main') || document.body;
    for (let ancestor = user.node; ancestor && ancestor !== region; ancestor = ancestor.parentElement) {
      const following = [];
      for (let sibling = ancestor.nextElementSibling; sibling; sibling = sibling.nextElementSibling) {
        if (hidden(sibling)) continue;
        const content = markdown(sibling).replace(/\n{3,}/g, '\n\n').trim();
        if (!content || isNonAnswerUiText(content)) continue;
        const tool = sibling.closest('[data-testid*="tool-call" i],[data-tool-call-id],span[class*="tool-message"]');
        if (tool) continue;
        following.push({ node: sibling, kind: 'answer', content,
          messageId: sibling.closest('[data-message-id]')?.getAttribute('data-message-id') || '' });
      }
      if (following.length) return following.slice(0, 16);
    }
    return [];
  }
  function readDiagnostics(user) {
    const afterUser = (selector) => user?.node ? queryAll(selector).filter((node) => {
      const relation = node.compareDocumentPosition(user.node);
      return !(relation & Node.DOCUMENT_POSITION_DISCONNECTED)
        && Boolean(relation & Node.DOCUMENT_POSITION_PRECEDING) && !hidden(node);
    }) : [];
    return `assistantRoles=${afterUser('[data-message-author-role="assistant" i]').length}; `
      + `assistantTurns=${afterUser('[data-turn="assistant" i]').length}; `
      + `conversationTurns=${afterUser('[data-testid^="conversation-turn"]').length}; `
      + `markdown=${afterUser('.markdown').length}; visibleTextFallback=${afterUser(ASSISTANT_TEXT_FALLBACK).length}; `
      + `parsedParts=${readParts(user).length}; rolelessSiblings=${rolelessAnswerParts(user).length}; `
      + `shadowRoots=${queryAll('*').filter((node) => node.shadowRoot).length}; `
      + `bodyTextLength=${normalize(document.body?.innerText).length}; ${userScanDiagnostics()}`;
  }
  globalThis.ChatCmdTranscript = Object.freeze({ latestUser, userText, readParts, readDiagnostics, normalize,
    isFooterDisclaimer, isActivityStatus, isNonAnswerUiText, conversationId, userScanDiagnostics });
})();
