import { describe, expect, it } from 'vitest';
import { prepareChatGptMessage } from './messageAttachments';

describe('prepareChatGptMessage', () => {
  it('sends the raw trimmed message when nothing is attached', () => {
    expect(prepareChatGptMessage('  Xin chào  ', {})).toBe('Xin chào');
  });

  it('adds only the selected plugin', () => {
    expect(prepareChatGptMessage('Làm việc này', { pluginName: 'rust_test' }))
      .toBe('plugin @rust_test\n\nrequest: Làm việc này');
  });

  it('adds only the selected project folder', () => {
    expect(prepareChatGptMessage('Làm việc này', { projectFolder: ' D:\\DEV\\CmdGPT ' }))
      .toBe('Project folder: D:\\DEV\\CmdGPT\n\nrequest: Làm việc này');
  });

  it('adds plugin and project in the requested order', () => {
    expect(prepareChatGptMessage('Làm việc này', { pluginName: 'rust_test', projectFolder: 'D:\\DEV\\CmdGPT' }))
      .toBe('plugin @rust_test\nProject folder: D:\\DEV\\CmdGPT\n\nrequest: Làm việc này');
  });
});
