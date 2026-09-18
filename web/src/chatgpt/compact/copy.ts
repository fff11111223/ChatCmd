import { appLocale } from '../../i18n';

const copy = {
  checking: ['Checking saved compact progress…', 'Checking saved compact progress…'],
  loadError: ['Could not read compact progress. Retry before sending.', 'Could not read compact progress. Retry before sending.'],
  waiting: ['Progress is saved. You can reload this page without losing this job.', 'Progress is saved. You can reload this page without losing this compact job.'],
  extensionMissing: ['Progress is saved, but the extension has not acknowledged it. Enable or update ChatCMD ChatGPT Bridge, reload this page, then choose Resume via extension.', 'Progress is saved, but the extension has not acknowledged it. Enable or update ChatCMD ChatGPT Bridge, reload the page, then choose Resume via extension.'],
  waking: ['Contacting the extension…', 'Contacting the extension…'],
  acknowledged: ['Waiting for ChatGPT. The extension will continue from the saved checkpoint.', 'Waiting for ChatGPT. The extension will continue from the saved checkpoint.'],
  resume: ['Resume via extension', 'Tiếp tục qua extension'],
  cancelJob: ['Cancel compact', 'Hủy thu gọn'],
  cancelled: ['Cancelled', 'Cancelled'],
  completed: ['Completed', 'Hoàn tất'],
  completedDetail: ['The new chat is ready. This task, its history and queued messages are unchanged.', 'The new chat is ready. This task, its history and queued messages are unchanged.'],
  cancelledDetail: ['Compact was cancelled. Your draft and queued messages are preserved.', 'Compact was cancelled. Your draft and queued messages are preserved.'],
  preserved: ['Sending is paused. Your draft and queued messages are preserved.', 'Sending is paused. Your draft and queued messages are preserved.'],
  conflict: ['Progress changed before cancellation. Review the latest state and try again.', 'Progress changed before cancellation. Review the latest state and try again.'],
  empty: ['No compact history yet.', 'Chưa có lần thu gọn ngữ cảnh nào.'],
  reference: ['Open old conversation', 'Mở cuộc trò chuyện cũ'],
  newTab: ['(reference only, opens in a new tab)', '(reference only, opens in a new tab)'],
  oldId: ['Old conversation', 'Cuộc trò chuyện cũ'],
  newId: ['New conversation', 'Cuộc trò chuyện mới'],
  details: ['Conversation details', 'Chi tiết cuộc trò chuyện'],
  noUrl: ['Old conversation link unavailable.', 'Chưa có liên kết cuộc trò chuyện cũ.'],
  confirm: ['Confirm compact', 'Xác nhận thu gọn'],
  continueAfterCompact: ['Continue working after compact completes', 'Tiếp tục công việc sau khi compact xong'],
  continueHint: ['Off by default. Only when checked will ChatCMD send the continuation message after attaching the new chat.', 'Off by default. Only when checked will ChatCMD send the continuation message after attaching the new chat.'],
  preparing: ['Preparing the existing conversation and its context.', 'Preparing the existing conversation and its context.'],
  writing_handoff: ['ChatGPT is writing the handoff. Keep the ChatGPT tab available.', 'ChatGPT is writing the handoff. Keep the ChatGPT tab available.'],
  saving_handoff: ['Saving the handoff before opening a new chat.', 'Saving the handoff before opening a new chat.'],
  opening_new_chat: ['Opening the new chat and reconnecting it to this same task.', 'Opening the new chat and reconnecting it to this same task.'],
  bridgeSync: ['Waiting for the new conversation link before resuming messages…', 'Waiting for the new conversation link before resuming messages…'],
} as const;

export function compactText(key: keyof typeof copy): string {
  return copy[key][appLocale().toLowerCase().startsWith('vi') ? 1 : 0];
}
