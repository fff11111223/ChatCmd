const GEMINI_HOME = 'https://gemini.google.com/app';

function isGeminiUrl(url) {
  try {
    return new URL(url || '').origin === 'https://gemini.google.com';
  } catch {
    return false;
  }
}

async function findGeminiTab(targetUrl) {
  const tabs = await chrome.tabs.query({ url: 'https://gemini.google.com/*' });
  if (!tabs || tabs.length === 0) return null;
  if (targetUrl) {
    try {
      const target = new URL(targetUrl);
      const match = tabs.find((t) => {
        try {
          const u = new URL(t.url || '');
          return u.origin === target.origin && u.pathname === target.pathname;
        } catch {
          return false;
        }
      });
      if (match) return match;
    } catch {}
  }
  return tabs.find((tab) => tab.id && tab.status === 'complete') || tabs.find((tab) => tab.id) || null;
}

async function acquireGeminiTab(targetUrl) {
  const existing = await findGeminiTab(targetUrl);
  if (existing?.id) {
    if (targetUrl && existing.url !== targetUrl) {
      try {
        const u = new URL(existing.url || '');
        const t = new URL(targetUrl);
        if (u.pathname !== t.pathname) {
          await chrome.tabs.update(existing.id, { url: targetUrl });
        }
      } catch {}
    }
    return existing;
  }
  const url = targetUrl || GEMINI_HOME;
  return await chrome.tabs.create({ url, active: false });
}

async function waitForTabComplete(tabId) {
  for (let i = 0; i < 40; i += 1) {
    try {
      const tab = await chrome.tabs.get(tabId);
      if (tab.status === 'complete') return tab;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

async function waitForGeminiContent(tabId) {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      const result = await chrome.tabs.sendMessage(tabId, { type: 'chatcmd-content-alive', kind: 'gemini' });
      if (result?.ok && result.kind === 'gemini') return result;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error('Gemini content script is not ready. Reload the Gemini tab after reloading the extension.');
}

async function sendToGemini(tabId, payload) {
  await waitForGeminiContent(tabId);
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      return await chrome.tabs.sendMessage(tabId, payload);
    } catch (error) {
      if (attempt === 4) throw error;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  }
}

async function geminiTabStatus(conversationUrl) {
  const tab = await findGeminiTab(conversationUrl);
  if (!tab?.id) {
    return {
      chatGptTabOpen: false,
      conversationTabOpen: false,
      conversationReady: false,
    };
  }
  let ready = false;
  try {
    const res = await sendToGemini(tab.id, { type: 'chatcmd-gemini-ready' });
    ready = res?.ready === true;
  } catch {
    ready = tab.status === 'complete';
  }
  return {
    chatGptTabOpen: true,
    conversationTabOpen: true,
    conversationReady: ready,
    tabId: tab.id,
    tabUrl: tab.url,
  };
}

async function openGeminiTab(conversationUrl) {
  const target = conversationUrl || GEMINI_HOME;
  const existing = await findGeminiTab(target);
  if (existing?.id) return existing;
  return await chrome.tabs.create({ url: target, active: false });
}

async function focusGeminiTab(conversationUrl) {
  const target = conversationUrl || GEMINI_HOME;
  const tab = await findGeminiTab(target);
  if (!tab?.id) throw new Error('The Gemini tab for this conversation is no longer open.');
  await chrome.tabs.update(tab.id, { active: true });
  if (tab.windowId) await chrome.windows.update(tab.windowId, { focused: true });
}

async function closeGeminiTab(conversationUrl) {
  const tab = await findGeminiTab(conversationUrl);
  if (tab?.id) await chrome.tabs.remove(tab.id);
}

async function startGeminiRequest(message) {
  if (!message.requestId || !message.submittedContent) throw new Error('Invalid Gemini send request.');
  const tab = await acquireGeminiTab(message.conversationUrl);
  await waitForTabComplete(tab.id);
  const conversationUrl = tab.url || message.conversationUrl || GEMINI_HOME;
  const conversationId = ('GEMINI:' + new URL(conversationUrl).pathname).slice(0, 500);

  await postJson(
    message.localBaseUrl,
    `/api/local/chatgpt/bridge/${encodeURIComponent(message.requestId)}/started`,
    {
      conversationId,
      conversationUrl,
      model: message.model || 'Auto',
    }
  );

  await chrome.storage.session.set({
    [`chatcmd-request:${message.requestId}`]: {
      localBaseUrl: message.localBaseUrl,
      tabId: tab.id,
      conversationUrl,
      conversationId,
    },
  });

  await sendToGemini(tab.id, {
    type: 'chatcmd-gemini-run',
    requestId: message.requestId,
    submittedContent: message.submittedContent,
    model: message.model || 'Auto',
  });
  return tab;
}

async function handleGeminiResult(message, sender) {
  if (!message?.requestId) return;
  const key = `chatcmd-request:${message.requestId}`;
  const context = await chrome.storage.session.get(key);
  const request = context[key];
  if (!request || (sender.tab?.id && request.tabId !== sender.tab?.id)) return;

  if (message.error) {
    try {
      await postJson(
        request.localBaseUrl,
        `/api/local/chatgpt/bridge/${encodeURIComponent(message.requestId)}/result`,
        {
          status: 'failed',
          errorMessage: message.error,
        }
      );
    } catch {}
    await chrome.storage.session.remove(key);
    return;
  }

  try {
    await postJson(
      request.localBaseUrl,
      `/api/local/chatgpt/bridge/${encodeURIComponent(message.requestId)}/result`,
      {
        status: 'completed',
        conversationId: message.conversationId || request.conversationId,
        conversationUrl: message.conversationUrl || request.conversationUrl,
        assistantContent: message.assistantContent || '',
      }
    );
  } catch {}
  await chrome.storage.session.remove(key);
}

async function handleGeminiDiscovery(message, sender) {
  const tabId = sender.tab?.id;
  if (!tabId || !message?.conversationUrl) return;
  const conversationUrl = message.conversationUrl;
  const conversationId = message.conversationId || conversationIdFromUrl(conversationUrl);
  if (conversationId) {
    await bindConversationTab(conversationId, tabId, {
      localBaseUrl: approvalBaseUrl,
      conversationUrl,
    });
  }
}

async function handleGeminiTabNavigation(tabId, tabUrl) {
  if (!tabId || !tabUrl) return;
  const id = conversationIdFromUrl(tabUrl);
  if (id) {
    await bindConversationTab(id, tabId, {
      localBaseUrl: approvalBaseUrl,
      conversationUrl: tabUrl,
    });
  }
}

async function reconcileOpenGeminiIdentities() {
  try {
    const tabs = await chrome.tabs.query({ url: 'https://gemini.google.com/*' });
    for (const tab of tabs) {
      if (!tab?.id || !tab.url) continue;
      await handleGeminiTabNavigation(tab.id, tab.url);
      await refreshConversationAliases(tab.id, tab.url);
      await syncRequestIdentityFromTab(tab.id, tab.url);
    }
  } catch (error) {
    console.warn('[ChatCMD] Could not reconcile Gemini tabs on startup', error);
  }
}
