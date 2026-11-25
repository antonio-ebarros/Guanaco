import { $, $$, fmtGb, invoke, listen, state, switchView } from './js/state.js';
import { initPreferences } from './js/theme.js';
import { initProChatHeader, loadLicenseStatus, renderChatPersonaSelect } from './js/pro.js';
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
  renderBubbleSources,
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

function openAboutModal() {
  const modal = $('#about-modal');
  if (modal) modal.hidden = false;
}
function closeAboutModal() {
  const modal = $('#about-modal');
  if (modal) modal.hidden = true;
}
$('#btn-about')?.addEventListener('click', openAboutModal);
$('#brand-logo')?.addEventListener('click', openAboutModal);
$('#btn-close-about')?.addEventListener('click', closeAboutModal);
$('#about-modal')?.addEventListener('click', ev => {
  if (ev.target.id === 'about-modal') closeAboutModal();
});
window.addEventListener('keydown', ev => {
  if (ev.key === 'Escape') closeAboutModal();
});

document.addEventListener('click', ev => {
  const link = ev.target?.closest?.('a[href]');
  if (!link) return;
  const href = link.getAttribute('href') || '';
  if (/^(https?:\/\/|mailto:)/i.test(href)) {
    ev.preventDefault();
    invoke('open_external_url', { url: href }).catch(() => {});
  }
});

async function checkForUpdates(manual = false) {
  const statusEl = $('#about-update-status');
  const linkEl = $('#about-update-link');
  const btnAbout = $('#btn-about');
  if (manual && statusEl) {
    statusEl.hidden = false;
    statusEl.textContent = t('aboutCheckingUpdate');
  }
  if (linkEl) linkEl.hidden = true;

  try {
    const info = await invoke('check_app_update');
    if (info?.hasUpdate) {
      btnAbout?.classList.add('btn-has-update');
      if (statusEl) {
        statusEl.hidden = false;
        statusEl.textContent = `${t('aboutUpdateFound')} v${info.latestVersion}`;
      }
      if (linkEl) {
        linkEl.href = info.downloadUrl || info.releaseUrl;
        linkEl.textContent = `⬇ ${t('aboutDownloadUpdate')} (v${info.latestVersion})`;
        linkEl.hidden = false;
      }
    } else if (manual && statusEl) {
      statusEl.hidden = false;
      statusEl.textContent = `${t('aboutUpToDate')} (v${info?.currentVersion || '1.0.0'})`;
    }
  } catch {
    if (manual && statusEl) {
      statusEl.hidden = false;
      statusEl.textContent = t('aboutUpdateErr');
    }
  }
}

$('#btn-check-update')?.addEventListener('click', () => checkForUpdates(true));

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
  const { convId, delta, done, error, sources } = ev.payload;
  const conv = state.conversations.find(c => c.id === convId);
  if (!conv) return;
  const last = conv.messages[conv.messages.length - 1];
  if (!last || last.role !== 'assistant') return;
  if (Array.isArray(sources) && sources.length) {
    last.sources = sources;
    if (convId === state.activeConvId && state.activeStreamBubble) {
      renderBubbleSources(state.activeStreamBubble, sources);
    }
  }
  if (error) last.content += `\n\nErro: ${error}`;
  else if (delta) last.content += delta;

  if (String(last.content || '').trim()) {
    stopThinkingTimer();
  }

  if (convId === state.activeConvId && state.activeStreamBubble) {
    state.activeStreamBubble.dataset.rawText = last.content;
    const body = state.activeStreamBubble.querySelector('.msg-body');
    if (String(last.content || '').trim()) {
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
  renderChatPersonaSelect();
  if (!state.busy) renderChat();
});
initProChatHeader(refreshControl);
initModelsEvents();
initChatEvents();
loadLicenseStatus().then(() => bootstrap()).then(loadConversations);
setTimeout(() => checkForUpdates(false), 2500);
