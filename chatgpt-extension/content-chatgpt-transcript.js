// Read only ChatGPT's public rendered transcript. Never inspect private model/reasoning state.
(() => {
  const USERS = '[data-message-author-role="user" i],[data-turn="user" i],[data-testid^="user-message"]';
  const USER_TEXT_FALLBACK = '.text-size-chat.whitespace-pre-wrap';
  const ASSISTANT_TEXT_FALLBACK = '.text-size-chat.whitespace-pre-wrap';
  const TURNS = '[data-testid^="conversation-turn"], [data-turn="assistant"], [data-message-author-role="assistant"], .markdown, '
    + ASSISTANT_TEXT_FALLBACK;
  // aria-hidden only removes a node from the accessibility tree; it does not mean
  // the user cannot see it. Filter by actual rendered visibility below instead.
  const EXCLUDED = 'button,svg,script,style,.sr-only,.visually-hidden,[hidden],[role="alert"],'
    + '[data-chatcmd-ui],.chatcmd-return-button,.chatcmd-approval-panel,.clf-stream,.clf-stage,'
    + 'input,textarea,[contenteditable="true"],[data-testid*="composer" i],'
    + '[data-testid*="tool-call"],[data-tool-call-id],span[class*="tool-message"],'
    + '[aria-label="Open tool call list" i]';
  const TOOL_CALL_WRAPPERS = '[data-testid*="tool-call" i],[data-tool-call-id],span[class*="tool-message"]';
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
  function isNonAnswerUiText(text) {
    const value = normalize(text);
    // ChatGPT briefly renders an ellipsis-only assistant bubble while creating
    // a response. It is a loading placeholder, not a final answer to capture.
    return isFooterDisclaimer(value) || isActivityStatus(value) || /^(?:\.{3,}|…+|⋯)$/.test(value);
  }
  function queryAll(selector, root = document) {
    const found = [...root.querySelectorAll(selector)];
    for (const element of [...root.querySelectorAll('*')]) {
      if (element.shadowRoot) found.push(...queryAll(selector, element.shadowRoot));
    }
    return [...new Set(found)];
  }
  function composedParent(node) {
    const parent = node?.parentNode;
    return parent?.nodeType === 11 && parent.host ? parent.host : parent;
  }
  function composedContains(ancestor, node) {
    for (let current = node; current; current = composedParent(current)) {
      if (current === ancestor) return true;
    }
    return false;
  }
  function composedChildren(node) {
    return node?.shadowRoot ? [...node.shadowRoot.childNodes] : [...(node?.childNodes || [])];
  }
  function compareRenderedOrder(left, right) {
    if (left === right) return 0;
    const relation = left.compareDocumentPosition(right);
    if (!(relation & Node.DOCUMENT_POSITION_DISCONNECTED)) {
      if (relation & Node.DOCUMENT_POSITION_FOLLOWING) return -1;
      if (relation & Node.DOCUMENT_POSITION_PRECEDING) return 1;
    }
    const path = (node) => {
      const nodes = [];
      for (let current = node; current; current = composedParent(current)) nodes.push(current);
      return nodes;
    };
    const leftPath = path(left);
    const rightPath = path(right);
    let li = leftPath.length - 1;
    let ri = rightPath.length - 1;
    while (li >= 0 && ri >= 0 && leftPath[li] === rightPath[ri]) { li--; ri--; }
    if (li >= 0 && ri >= 0) {
      const parent = leftPath[li + 1];
      const children = composedChildren(parent);
      const leftIndex = children.indexOf(leftPath[li]);
      const rightIndex = children.indexOf(rightPath[ri]);
      if (leftIndex >= 0 && rightIndex >= 0 && leftIndex !== rightIndex) return leftIndex < rightIndex ? -1 : 1;
    }
    const leftRect = left.getBoundingClientRect?.();
    const rightRect = right.getBoundingClientRect?.();
    if (leftRect && rightRect && (leftRect.top !== rightRect.top || leftRect.left !== rightRect.left)) {
      return leftRect.top !== rightRect.top
        ? (leftRect.top < rightRect.top ? -1 : 1)
        : (leftRect.left < rightRect.left ? -1 : 1);
    }
    return 0;
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
  function latestUser(preferredNode = null) {
    let users = queryAll(USERS)
      .filter((node) => !node.parentElement?.closest(USERS));
    const semantic = users.length > 0;
    if (!semantic) users = queryAll(USER_TEXT_FALLBACK)
      .filter((node) => !node.closest('form,[contenteditable="true"],[data-testid*="composer"]') && !hidden(node));
    // Some ChatGPT builds render both user and assistant bubbles with the same
    // `.text-size-chat` class and expose no semantic role. Once the assistant
    // starts replying, choosing the last matching node silently rebinds this
    // request to the assistant bubble. Keep the original submitted node while
    // it remains connected; the observer passes that node back on every scan.
    const preferredIsUserLike = preferredNode?.isConnected
      && (preferredNode.matches?.(USERS) || preferredNode.matches?.(USER_TEXT_FALLBACK));
    const node = preferredIsUserLike ? preferredNode : users.at(-1);
    if (!node) return null;
    const content = userText(node);
    const id = node.getAttribute('data-message-id') || node.querySelector('[data-message-id]')?.getAttribute('data-message-id')
      || 'dom-user:' + (node.getAttribute('data-turn-id') || node.getAttribute('data-testid')
        || Math.max(0, users.indexOf(node))) + ':' + fingerprint(content);
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
  function hiddenProtocolCandidate(node) {
    for (let current = node; current && current !== document.body; current = current.parentElement) {
      // Normal transcript extraction excludes tool cards. A strictly valid,
      // visibly labeled ChatCMD call can live inside one of those wrappers, so
      // protocol parsing checks its visibility separately below.
      if (current.matches?.(EXCLUDED) && !current.matches?.(TOOL_CALL_WRAPPERS)) return true;
      const style = getComputedStyle(current);
      if (current.hidden || style.display === 'none' || style.visibility === 'hidden') return true;
    }
    return false;
  }
  function markdown(node) {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent || '';
    if (node.nodeType === Node.DOCUMENT_FRAGMENT_NODE) return [...node.childNodes].map(markdown).join('');
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
  function protocolToolCallParts(roots, afterNode = null) {
    const candidates = [];
    const seen = new Set();
    for (const root of roots) {
      for (const block of queryAll('*', root)) {
        const text = block.textContent || '';
        if (text.length < 20 || text.length > 100_000 || !text.includes('"id"')
            || !text.includes('"tool"') || !text.includes('"arguments"')) continue;
        const start = text.indexOf('{');
        const end = text.lastIndexOf('}');
        if (start < 0 || end <= start) continue;
        const payload = text.slice(start, end + 1).trim();
        let parsed;
        try { parsed = JSON.parse(payload); } catch { continue; }
        if (typeof parsed?.id !== 'string' || !/^call_[\w-]+$/.test(parsed.id)
            || typeof parsed.tool !== 'string' || !parsed.arguments || typeof parsed.arguments !== 'object'
            || Array.isArray(parsed.arguments)) continue;
        // This page-wide scan exists to find assistant tool cards rendered
        // outside <main>. Never inspect the submitted user turn itself: some
        // layouts place protocol-looking code blocks inside the user's bubble,
        // and rendered-order comparisons are unreliable for nested/shadow DOM.
        if (afterNode && (composedContains(afterNode, block) || composedContains(block, afterNode)
            || block.closest(USERS) || compareRenderedOrder(afterNode, block) >= 0)) continue;
        const nestedCode = block.matches('pre') ? block.querySelector('code') : null;
        const source = nestedCode || block;
        const classNames = `${block.className || ''} ${source.className || ''}`;
        const language = classNames.match(/\blanguage-([\w-]+)/i)?.[1]?.toLowerCase()
          || block.closest('[data-language]')?.getAttribute('data-language')?.toLowerCase() || '';
        let labelNode = block;
        let labeledCall = language === 'chatcmd_tool_call';
        for (let depth = 0; labelNode && depth < 6 && !labeledCall; depth++, labelNode = composedParent(labelNode)) {
          labeledCall = /chatcmd_tool_call/i.test(labelNode.textContent || '')
            || /chatcmd_tool_call/i.test(labelNode.getAttribute?.('aria-label') || '')
            || /chatcmd_tool_call/i.test(labelNode.getAttribute?.('data-language') || '');
        }
        if (!labeledCall || hiddenProtocolCandidate(block)) continue;
        if (seen.has(payload)) continue;
        candidates.push({ node: block, payload,
          messageId: block.closest('[data-message-id]')?.getAttribute('data-message-id') || '' });
      }
    }
    const smallest = candidates.filter((candidate) => !candidates.some((other) => other !== candidate
      && candidate.node.contains(other.node)));
    return smallest.filter((candidate) => {
      if (seen.has(candidate.payload)) return false;
      seen.add(candidate.payload);
      return true;
    }).map(({ node, payload, messageId }) => {
      const fence = '`'.repeat(Math.max(3, ...[...payload.matchAll(/`+/g)].map((match) => match[0].length + 1)));
      let callId = '';
      try { callId = JSON.parse(payload).id || ''; } catch { /* already validated above */ }
      return { node, kind: 'answer', content: `${fence}chatcmd_tool_call\n${payload}\n${fence}`,
        messageId: callId ? `tool-call:${callId}` : messageId };
    });
  }
  function readParts(user) {
    if (!user?.node.isConnected) return [];
    const roots = queryAll(`${TURNS},[data-message-author-role="assistant"]`)
      .filter((node) => {
        // compareRenderedOrder(left, right) is negative when `left` appears first.
        // Role-less fallback selectors also match user bubbles. Never treat the
        // submitted bubble, or any nested fragment inside it, as an assistant turn.
        if (node === user.node || node.contains(user.node) || user.node.contains(node)) return false;
        return compareRenderedOrder(user.node, node) < 0;
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
    const parts = [...candidates].sort(compareRenderedOrder)
      .map((node) => ({ node, kind: node.closest('[data-interrupted]:not([data-interrupted="false"])') ? 'commentary' : 'answer',
        content: markdown(node).replace(/\n{3,}/g, '\n\n').trim(),
        messageId: node.closest('[data-message-id]')?.getAttribute('data-message-id') || '' }))
      .filter((part) => part.content && !isNonAnswerUiText(part.content));
    const rolelessParts = rolelessAnswerParts(user);
    const protocolParts = [
      ...protocolToolCallParts(roots),
      // ChatGPT may render the visible tool card outside the conversation <main>.
      // Scan the page, while still requiring the block to follow this exact user turn.
      ...protocolToolCallParts([document.body], user.node),
    ];
    const toolCalls = protocolParts.filter((part, index, all) =>
      all.findIndex((candidate) => candidate.content === part.content) === index);
    // Prefer one transcript source. Combining semantic turns, broad page scans,
    // and a range-to-composer tail duplicates the same rendered answer as nested
    // fragments. Fall back only when the preceding source found no answer text.
    let selectedAnswers = parts.filter((part) => part.kind === 'answer');
    if (!selectedAnswers.length) selectedAnswers = rolelessParts.filter((part) => part.kind === 'answer');
    if (!selectedAnswers.length) selectedAnswers = genericRenderedAnswerParts(user).filter((part) => part.kind === 'answer');
    if (!selectedAnswers.length) {
      const tailPart = renderedTailPart(user, protocolParts);
      if (tailPart) selectedAnswers = [tailPart];
    }
    const answerParts = [...selectedAnswers]
      .map((part) => ({ ...part, content: stripToolCallBlocks(part.content, toolCalls).trim() }))
      .filter((part) => part.content);
    const exactUnique = [...answerParts, ...toolCalls]
      .filter((part, index, all) => all.findIndex((candidate) => candidate.content === part.content) === index);
    const uniqueToolCalls = exactUnique.filter((part) => /```+\s*chatcmd_tool_call\b/i.test(part.content));
    const answers = exactUnique.filter((part) => !uniqueToolCalls.includes(part));
    const comparable = (content) => normalize(content).replace(/[ `*_#>|]/g, '');
    const nonOverlapping = answers.filter((part) => {
      const text = comparable(part.content);
      if (text.length < 5) return true;
      return !answers.some((other) => other !== part && comparable(other.content).length > text.length
        && comparable(other.content).includes(text));
    });
    return [...nonOverlapping, ...uniqueToolCalls].sort((a, b) => compareRenderedOrder(a.node, b.node));
  }
  function stripToolCallBlocks(content, protocolParts) {
    let result = content;
    for (const part of protocolParts) {
      const payload = part.content.match(/```+chatcmd_tool_call\n([\s\S]*?)\n```+/i)?.[1];
      if (!payload) continue;
      const escaped = payload.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const payloadBlock = new RegExp('(?:^|\\n)\\s*(?:`+\\s*chatcmd_tool_call\\s*\\n)?'
        + escaped + '(?:\\n`+)?(?=\\n|$)', 'gi');
      result = result.replace(payloadBlock, '\n');
    }
    // A protocol block can be rendered as an empty fence after its payload has
    // been removed. Remove only empty tool-call fences and their standalone label.
    return result
      .replace(/(^|\n)\s*`{1,}\s*chatcmd_tool_call\s*\n\s*`{1,}\s*(?=\n|$)/gi, '$1')
      .replace(/(^|\n)\s*chatcmd_tool_call\s*(?=\n|$)/gi, '$1')
      .replace(/\n{3,}/g, '\n\n');
  }
  function renderedTailPart(user, protocolParts) {
    if (!user?.node?.isConnected) return null;
    const root = user.node.getRootNode();
    const isShadowRoot = typeof ShadowRoot !== 'undefined' && root instanceof ShadowRoot;
    const region = isShadowRoot ? root : (user.node.closest('main,[role="main"]') || document.body);
    if (region !== root && !region.contains(user.node)) return null;
    try {
      const range = document.createRange();
      range.setStartAfter(user.node);
      const boundaries = queryAll(`${USERS},form,[contenteditable="true"],[data-testid*="composer" i]`, region)
        .filter((node) => node !== user.node && node.getRootNode() === root
          && compareRenderedOrder(user.node, node) < 0 && !hidden(node))
        .sort(compareRenderedOrder);
      if (boundaries[0]) range.setEndBefore(boundaries[0]);
      else {
        range.selectNodeContents(region);
        range.setStartAfter(user.node);
      }
      let content = stripToolCallBlocks(markdown(range.cloneContents()), protocolParts)
        .replace(/\n{3,}/g, '\n\n').trim();
      content = content.split('\n').filter((line) => !isNonAnswerUiText(line)).join('\n').trim();
      return content && !isNonAnswerUiText(content)
        ? { node: region, kind: 'answer', content, messageId: `rendered-tail:${user.id}` }
        : null;
    } catch { return null; }
  }
  function rolelessAnswerParts(user) {
    if (!user?.node?.isConnected) return [];
    const region = user.node.closest('main,[role="main"]') || document.body;
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
  function genericRenderedAnswerParts(user) {
    if (!user?.node?.isConnected) return [];
    // Read rendered answer content from the whole page. ChatGPT has moved response
    // and tool-card nodes outside <main> in some layouts; filters below keep UI out.
    const region = document.body;
    const selector = '[data-message-id],[data-testid^="conversation-turn"],[data-turn="assistant" i],'
      + '.markdown,[class*="markdown" i],' + ASSISTANT_TEXT_FALLBACK
      + ',article,section,div,p,pre,code,span,li,blockquote,h1,h2,h3,h4,h5,h6,table';
    const candidates = queryAll(selector, region).filter((node) => {
      if (node === user.node || node.contains(user.node) || user.node.contains(node) || hidden(node)) return false;
      if (node.tagName === 'DIV' && !node.matches(ASSISTANT_TEXT_FALLBACK)
          && ![...node.childNodes].some((child) =>
        child.nodeType === Node.TEXT_NODE && normalize(child.textContent))) return false;
      if (compareRenderedOrder(user.node, node) >= 0) return false;
      if (node.closest('form,[contenteditable="true"],[data-testid*="composer" i],nav,header,footer,aside')) return false;
      if (node.closest('[data-testid*="tool-call" i],[data-tool-call-id],span[class*="tool-message"]')) return false;
      return true;
    });
    const candidateSet = new Set(candidates);
    const topLevel = candidates.filter((node) => {
      for (let parent = node.parentElement; parent && parent !== region; parent = parent.parentElement) {
        if (candidateSet.has(parent)) return false;
      }
      return true;
    });
    return topLevel.map((node) => ({ node, kind: 'answer',
      content: markdown(node).replace(/\n{3,}/g, '\n\n').trim(),
      messageId: node.closest('[data-message-id]')?.getAttribute('data-message-id') || '' }))
      .filter((part) => part.content && !isNonAnswerUiText(part.content)).slice(0, 32);
  }
  function toolBlockDiagnostics(user) {
    if (!user?.node?.isConnected) return 'visiblePre=0; preBlocks=unavailable';
    const calls = protocolToolCallParts([document.body], user.node);
    const blocks = queryAll('pre', document.body).filter((pre) => {
      if (compareRenderedOrder(user.node, pre) >= 0) return false;
      for (let current = pre; current && current !== document.body; current = current.parentElement) {
        const style = getComputedStyle(current);
        if (current.hidden || style.display === 'none' || style.visibility === 'hidden') return false;
      }
      return true;
    });
    const summary = blocks.slice(-8).map((pre) => {
      const code = pre.querySelector('code');
      const classes = `${pre.className || ''} ${code?.className || ''}`;
      const language = classes.match(/\blanguage-([\w-]+)/i)?.[1]?.toLowerCase()
        || pre.getAttribute('data-language')?.toLowerCase() || 'none';
      const label = pre.closest('[data-language]')?.getAttribute('data-language')?.toLowerCase()
        || pre.parentElement?.getAttribute('aria-label')?.toLowerCase() || 'none';
      const text = code?.textContent || pre.textContent || '';
      const callShape = /"id"\s*:\s*"call_[^"]+"/.test(text) && /"tool"\s*:\s*"[^"]+"/.test(text);
      return `${language}/${label}:${text.length}${callShape ? ':callShape' : ''}`;
    }).join(',') || 'none';
    return `visiblePre=${blocks.length}; visibleCallBlocks=${calls.length}; callBlockLengths=${calls.map((part) => part.content.length).join(',') || 'none'}; preBlocks=${summary}`;
  }
  function readDiagnostics(user, capturedParts = null) {
    const afterUser = (selector) => user?.node ? queryAll(selector).filter((node) => {
      return compareRenderedOrder(user.node, node) < 0 && !hidden(node);
    }) : [];
    return `assistantRoles=${afterUser('[data-message-author-role="assistant" i]').length}; `
      + `assistantTurns=${afterUser('[data-turn="assistant" i]').length}; `
      + `conversationTurns=${afterUser('[data-testid^="conversation-turn"]').length}; `
      + `markdown=${afterUser('.markdown').length}; visibleTextFallback=${afterUser(ASSISTANT_TEXT_FALLBACK).length}; `
      + `parsedParts=${Array.isArray(capturedParts) ? capturedParts.length : readParts(user).length}; rolelessSiblings=${rolelessAnswerParts(user).length}; `
      + `genericRendered=${genericRenderedAnswerParts(user).length}; `
      + `${toolBlockDiagnostics(user)}; `
      + `shadowRoots=${queryAll('*').filter((node) => node.shadowRoot).length}; `
      + `bodyTextLength=${normalize(document.body?.innerText).length}; ${userScanDiagnostics()}`;
  }
  globalThis.ChatCmdTranscript = Object.freeze({ latestUser, userText, readParts, readDiagnostics, normalize,
    isFooterDisclaimer, isActivityStatus, isNonAnswerUiText, conversationId, userScanDiagnostics, toolBlockDiagnostics });
})();
