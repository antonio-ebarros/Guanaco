import { $, $$, fmtGb, listen, state, switchView } from './js/state.js';
import { initPreferences } from './js/theme.js';
import { t } from './js/i18n.js';
import {
  bootstrap,
  initModelsEvents,
  refreshControl,
  renderControlView,
  renderEngineStatus,
  renderModelsView,
  setChatRenderer,
} from './js/models.js';
import {
  initChatEvents,
  loadConversations,
  persistConversations,
  renderChat,
  stopThinkingTimer,
} from './js/chat.js';

setChatRenderer(renderChat);

$$('.nav-tab').forEach(b =>
  b.addEventListener('click', () => switchView(b.dataset.view, refreshControl))
);

$('#btn-profile-pill')?.addEventListener('click', () =>
  switchView('control', refreshControl)
);

listen('pull-progress', ev => {
  const p = ev.payload;
  $('#pull-status').textContent = p.status || t('downloadingStatus');
  if (p.total > 0) {
    const pct = Math.round(p.percent);
    $('#pull-pct').textContent = `${pct}%`;
    $('#pull-fill').style.width = `${pct}%`;
  }
});

listen('chat-event', ev => {
  if (state.streamCancelled) return;
  const { convId, delta, done, error } = ev.payload;
  const conv = state.conversations.find(c => c.id === convId);
  if (!conv) return;
  const last = conv.messages[conv.messages.length - 1];
  if (!last || last.role !== 'assistant') return;
  if (error) last.content += `\n\nErro: ${error}`;
  else if (delta) last.content += delta;

  if (convId === state.activeConvId && state.activeStreamBubble) {
    state.activeStreamBubble.dataset.rawText = last.content;
    const body = state.activeStreamBubble.querySelector('.msg-body');
    if (String(last.content || '').trim()) {
      stopThinkingTimer();
      if (typeof window.renderMarkdown === 'function') body.innerHTML = window.renderMarkdown(last.content);
      else body.textContent = last.content;
    }
    $('#chat-log').scrollTop = $('#chat-log').scrollHeight;
  }
  if (done) {
    stopThinkingTimer();
    persistConversations();
  }
});

listen('engine-install-progress', ev => {
  const p = ev.payload;
  const st = $('#eng-install-status');
  const pctEl = $('#eng-install-pct');
  const fill = $('#eng-install-fill');
  if (!st) return;
  if (p.total > 0) {
    const pct = Math.round(p.percent);
    st.textContent = `${p.status} (${fmtGb(p.completed)} / ${fmtGb(p.total)})`;
    if (pctEl) pctEl.textContent = `${pct}%`;
    if (fill) fill.style.width = `${pct}%`;
  } else {
    st.textContent = p.status || 'Preparando…';
  }
});

listen('tts-install-progress', ev => {
  const p = ev.payload;
  const st = $('#tts-prog-status');
  const pctEl = $('#tts-prog-pct');
  const fill = $('#tts-prog-fill');
  if (!st) return;
  if (p.total > 0) {
    const pct = Math.round(p.percent);
    st.textContent = p.status;
    if (pctEl) pctEl.textContent = `${pct}%`;
    if (fill) fill.style.width = `${pct}%`;
  }
});

initPreferences(() => {
  renderEngineStatus();
  renderModelsView();
  renderControlView();
  if (!state.busy) renderChat();
});
initModelsEvents();
initChatEvents();
bootstrap().then(loadConversations);
