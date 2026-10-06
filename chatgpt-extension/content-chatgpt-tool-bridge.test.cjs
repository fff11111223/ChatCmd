const assert = require('node:assert/strict');
const test = require('node:test');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');

function setupBridge() {
  const context = {
    console,
    localStorage: { getItem: () => null },
    globalThis: {},
  };
  context.globalThis = context;
  context.ChatCmdRuntime = {
    sendMessage: async () => ({ ok: true }),
  };
  vm.createContext(context);
  const code = readFileSync(join(__dirname, 'content-chatgpt-tool-bridge.js'), 'utf8');
  vm.runInContext(code, context);
  return context.ChatCmdToolBridge;
}

test('parses tool calls whose arguments contain triple backticks', () => {
  const bridge = setupBridge();
  const text = [
    'I will write the markdown file for you:',
    '```chatcmd_tool_call',
    '{',
    '  "id": "call_1",',
    '  "tool": "fs_write_text",',
    '  "arguments": {',
    '    "path": "README.md",',
    '    "content": "# Project\\n\\n```python\\nprint(\\"hello world\\")\\n```\\n"',
    '  }',
    '}',
    '```',
  ].join('\n');

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 'call_1');
  assert.equal(calls[0].tool, 'fs_write_text');
  assert.equal(calls[0].arguments.path, 'README.md');
  assert.equal(
    calls[0].arguments.content,
    '# Project\n\n```python\nprint("hello world")\n```\n'
  );
});

test('parses multiple tool calls when one or more contain triple backticks', () => {
  const bridge = setupBridge();
  const text = [
    '```chatcmd_tool_call',
    '{"id":"call_10","tool":"fs_write_text","arguments":{"path":"a.md","content":"```\\ncode\\n```"}}',
    '```',
    'Now calling the second tool:',
    '```chatcmd_tool_call',
    '{"id":"call_11","tool":"fs_read_text","arguments":{"path":"b.txt"}}',
    '```',
  ].join('\n');

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 2);
  assert.equal(calls[0].id, 'call_10');
  assert.equal(calls[0].arguments.content, '```\ncode\n```');
  assert.equal(calls[1].id, 'call_11');
  assert.equal(calls[1].arguments.path, 'b.txt');
});

test('parses tool calls containing braces and escaped quotes inside strings', () => {
  const bridge = setupBridge();
  const text = [
    '```chatcmd_tool_call',
    '{',
    '  "id": "call_20",',
    '  "tool": "fs_write_text",',
    '  "arguments": {',
    '    "path": "test.js",',
    '    "content": "function test() { const s = \\\"{\\\\\\\"nested\\\\\\\": true}\\\"; return s; }"',
    '  }',
    '}',
    '```',
  ].join('\n');

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 'call_20');
  assert.match(calls[0].arguments.content, /function test\(\)/);
  assert.match(calls[0].arguments.content, /nested/);
});

test('parses tool calls wrapped in 4-backtick code blocks containing 3 backticks', () => {
  const bridge = setupBridge();
  const text = [
    '````chatcmd_tool_call',
    '{',
    '  "id": "call_30",',
    '  "tool": "fs_write_text",',
    '  "arguments": {',
    '    "content": "```json\\n{\\"test\\": 123}\\n```"',
    '  }',
    '}',
    '````',
  ].join('\n');

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 'call_30');
  assert.equal(calls[0].arguments.content, '```json\n{"test": 123}\n```');
});

test('parses inline [[CHATCMD_TOOL_CALL:{...}]] tags with backticks', () => {
  const bridge = setupBridge();
  const text = 'Result: [[CHATCMD_TOOL_CALL:{"id":"call_40","tool":"command_run","arguments":{"command":"echo \\"```test```\\""}}]]';

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 'call_40');
  assert.equal(calls[0].tool, 'command_run');
  assert.equal(calls[0].arguments.command, 'echo "```test```"');
});

test('generic fallback code block parses JSON containing backticks and ignores chatcmd_tool_result', () => {
  const bridge = setupBridge();
  const text = [
    'Previous result was:',
    '```chatcmd_tool_result',
    '{"id":"call_50","tool":"command_run","ok":true,"content":"ok"}',
    '```',
    'Generic tool call:',
    '```json',
    '{',
    '  "id": "call_51",',
    '  "tool": "fs_write_text",',
    '  "arguments": { "content": "```sh\\necho 1\\n```" }',
    '}',
    '```',
  ].join('\n');

  const calls = bridge.parseToolCalls(text);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 'call_51');
  assert.equal(calls[0].arguments.content, '```sh\necho 1\n```');
});

test('returns empty array when JSON is incomplete during streaming', () => {
  const bridge = setupBridge();
  const streamingText = [
    '```chatcmd_tool_call',
    '{',
    '  "id": "call_60",',
    '  "tool": "fs_write_text",',
    '  "arguments": {',
    '    "content": "```python\\nprint(1',
  ].join('\n');

  const calls = bridge.parseToolCalls(streamingText);
  assert.equal(calls.length, 0);
  assert.equal(bridge.hasPendingToolCalls(streamingText), false);
});
