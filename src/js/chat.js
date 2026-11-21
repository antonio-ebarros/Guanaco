import { $, invoke, state, switchView } from './state.js';
import { bootstrap, installEngine, startEngine } from './models.js';
import { copyResponseText, stopSpeaking, toggleSpeak } from './speech.js';
import { getResponseProfile } from './theme.js';
import { getEffectiveKnowledgeFolder, getEffectiveSystemPrompt, getProSettings } from './pro.js';
import { t } from './i18n.js';

let thinkingTimer = null;
const input = $('#input');

export async function loadConversations() {
  try {
    const raw = await invoke('load_conversations');
    state.conversations = JSON.parse(raw || '[]');
  } catch {
    state.conversations = [];
  }
  if (!state.conversations.length) newConversation();
  else {
    state.activeConvId = state.conversations[0].id;
    renderConvList();
    renderChat();
  }
}

export async function persistConversations() {
  await invoke('save_conversations', { data: JSON.stringify(state.conversations) });
}

export function activeConv() {
  return state.conversations.find(c => c.id === state.activeConvId) || state.conversations[0];
}

export function newConversation() {
  stopSpeaking();
  const c = {
    id: String(Date.now()),
    title: t('newConvTitle'),
    model: $('#model-select').value || state.installed[0]?.name || '',
    messages: [],
  };
  state.conversations.unshift(c);
  state.activeConvId = c.id;
  renderConvList();
  renderChat();
  switchView('chat');
}

export function renderConvList() {
  const nav = $('#conv-list');
  nav.innerHTML = '';
  for (const c of state.conversations) {
    const btn = document.createElement('button');
    btn.className = 'conv-item' + (c.id === state.activeConvId ? ' active' : '');
    btn.type = 'button';
    btn.innerHTML = `<span class="conv-name"></span><span class="conv-del" title="Apagar">✕</span>`;
    btn.querySelector('.conv-name').textContent = c.title;
    btn.onclick = ev => {
      if (ev.target.classList.contains('conv-del')) {
        state.conversations = state.conversations.filter(x => x.id !== c.id);
        if (!state.conversations.length) newConversation();
        else if (state.activeConvId === c.id) state.activeConvId = state.conversations[0].id;
        persistConversations();
        renderConvList();
        renderChat();
        return;
      }
      stopSpeaking();
      state.activeConvId = c.id;
      renderConvList();
      renderChat();
      switchView('chat');
    };
    nav.append(btn);
  }
}

export function renderThinkingHtml(label = 'Pensando', sec = 0) {
  return `<div class="thinking-indicator"><span class="thinking-dots"><span></span><span></span><span></span></span><span>${label}… ${sec}s</span></div>`;
}

export function stopThinkingTimer() {
  if (thinkingTimer) {
    clearInterval(thinkingTimer);
    thinkingTimer = null;
  }
}

function startThinkingTimer(label = 'Pensando') {
  stopThinkingTimer();
  state.thinkingLabel = label;
  state.thinkingStartedAt = Date.now();
  thinkingTimer = setInterval(() => {
    if (!state.activeStreamBubble || state.activeConvId !== state.streamingConvId) return;
    const sec = Math.floor((Date.now() - state.thinkingStartedAt) / 1000);
    const body = state.activeStreamBubble.querySelector('.msg-body');
    if (body && body.querySelector('.thinking-indicator')) {
      body.innerHTML = renderThinkingHtml(state.thinkingLabel || label, sec);
    }
  }, 1000);
}

export function renderBubbleSources(el, sources = []) {
  const box = el?.querySelector('.msg-sources');
  if (!box) return;
  if (!Array.isArray(sources) || !sources.length) {
    box.hidden = true;
    box.innerHTML = '';
    return;
  }
  box.hidden = false;
  box.innerHTML = `<span class="sources-label">📚 ${t('sourcesConsulted')}:</span>`;
  for (const s of sources) {
    const tag = document.createElement('span');
    tag.className = 'msg-source-tag';
    tag.textContent = s;
    box.append(tag);
  }
}

export function appendBubble(role, content, media = [], fileNames = [], thinkingLabel = null, sources = []) {
  const el = document.createElement('div');
  el.className = `msg ${role}`;
  el.dataset.rawText = content || '';

  const actionsHtml = role === 'assistant'
    ? `<div class="msg-actions">
         <button class="msg-action speak-btn" type="button" title="${t('speakTitle')}">${t('speak')}</button>
         <button class="msg-action copy-btn" type="button" title="${t('copyTitle')}">${t('copy')}</button>
       </div>`
    : '';

  el.innerHTML = `
    <div class="msg-head">
      <div class="msg-role">${role === 'user' ? t('you') : t('guanaco')}</div>
      ${actionsHtml}
    </div>
    <div class="msg-thumbs" hidden></div>
    <div class="msg-files" hidden></div>
    <div class="msg-sources" hidden></div>
    <div class="msg-body"></div>
  `;

  if (role === 'assistant') {
    const speakBtn = el.querySelector('.speak-btn');
    const copyBtn = el.querySelector('.copy-btn');
    speakBtn.onclick = () => toggleSpeak(speakBtn, el.dataset.rawText);
    copyBtn.onclick = () => copyResponseText(copyBtn, el.dataset.rawText);
    renderBubbleSources(el, sources);
  }

  const thumbs = el.querySelector('.msg-thumbs');
  if (Array.isArray(media) && media.length) {
    thumbs.hidden = false;
    for (const item of media) {
      const img = document.createElement('img');
      img.className = 'msg-thumb';
      img.alt = 'Imagem anexada';
      img.src = item.startsWith('data:') ? item : `data:image/jpeg;base64,${item}`;
      thumbs.append(img);
    }
  }
  const filesBox = el.querySelector('.msg-files');
  if (Array.isArray(fileNames) && fileNames.length) {
    filesBox.hidden = false;
    for (const name of fileNames) {
      const tag = document.createElement('span');
      tag.className = 'msg-file-tag';
      tag.textContent = `📄 ${name}`;
      filesBox.append(tag);
    }
  }
  const body = el.querySelector('.msg-body');
  if (thinkingLabel && !String(content || '').trim()) {
    body.innerHTML = renderThinkingHtml(thinkingLabel, 0);
  } else if (typeof window.renderMarkdown === 'function') {
    body.innerHTML = window.renderMarkdown(content || '');
  } else {
    body.textContent = content || '';
  }
  $('#chat-log').append(el);
  return el;
}

export function renderChat() {
  const conv = activeConv();
  $('#chat-title').textContent = conv?.title || t('newConvTitle');
  const log = $('#chat-log');
  log.innerHTML = '';

  if (!state.sysInfo?.ollamaRunning) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = state.sysInfo?.ollamaInstalled
      ? `<h2>${t('engineOffTitle')}</h2>
         <p>${t('engineOffDesc')}</p>
         <button id="wb-start" class="btn-primary" type="button">${t('startEngineNow')}</button>`
      : `<h2>${t('welcomeTitle')}</h2>
         <p>${t('welcomeDesc')}</p>
         <div id="eng-install-box" hidden style="display:flex;flex-direction:column;gap:8px;">
           <div class="row-between">
             <strong id="eng-install-status">${t('preparing')}</strong>
             <span id="eng-install-pct">0%</span>
           </div>
           <div class="progress-track"><div id="eng-install-fill" class="progress-fill"></div></div>
         </div>
         <div style="display:flex;gap:8px;flex-wrap:wrap;">
           <button id="wb-install" class="btn-primary" type="button">${t('prepareAutoBtn')}</button>
           <button id="wb-recheck" class="btn-ghost" type="button">${t('recheckBtn')}</button>
         </div>`;
    log.append(box);
    box.querySelector('#wb-start')?.addEventListener('click', startEngine);
    box.querySelector('#wb-recheck')?.addEventListener('click', bootstrap);
    box.querySelector('#wb-install')?.addEventListener('click', installEngine);
    return;
  }

  if (!state.installed.length) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = `
      <h2>${t('chooseFirstTitle')}</h2>
      <p>${t('chooseFirstDesc', { ram: state.sysInfo.totalRamGb })}</p>
      <button id="wb-go-models" class="btn-primary" type="button">${t('chooseFirstBtn')}</button>
    `;
    log.append(box);
    box.querySelector('#wb-go-models').onclick = () => switchView('models');
    return;
  }

  if (!conv || !conv.messages.length) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = `
      <h2>${t('allReadyTitle')}</h2>
      <p>${t('allReadyDesc', { model: $('#model-select').value || 'modelo local' })}</p>
    `;
    log.append(box);
    return;
  }

  state.activeStreamBubble = null;
  conv.messages.forEach((m, idx) => {
    const isStreamingThis =
      state.busy &&
      state.streamingConvId === conv.id &&
      idx === conv.messages.length - 1 &&
      m.role === 'assistant';
    const isThinking = isStreamingThis && !String(m.content || '').trim();
    const lbl = isThinking ? (state.thinkingLabel || t('thinking')) : null;
    const bubble = appendBubble(
      m.role,
      m.displayContent ?? m.content,
      m.previews || m.images || [],
      m.fileNames || [],
      lbl,
      m.sources || []
    );
    if (m.cancelled) {
      const body = bubble.querySelector('.msg-body');
      if (body) body.innerHTML = `<em class="muted">${m.content || t('cancelledMsg')}</em>`;
    }
    if (isStreamingThis) {
      state.activeStreamBubble = bubble;
      if (isThinking && state.thinkingStartedAt > 0) {
        const sec = Math.floor((Date.now() - state.thinkingStartedAt) / 1000);
        const body = bubble.querySelector('.msg-body');
        if (body) body.innerHTML = renderThinkingHtml(lbl, sec);
      }
    }
  });
  log.scrollTop = log.scrollHeight;
}

// ---- Editor Markdown ao vivo ----
function placeCaret(el, atEnd = true) {
  const selObj = window.getSelection();
  const range = document.createRange();
  range.selectNodeContents(el);
  range.collapse(!atEnd);
  selObj.removeAllRanges();
  selObj.addRange(range);
}

function currentBlock() {
  const selObj = window.getSelection();
  if (!selObj || !selObj.rangeCount) return null;
  let n = selObj.anchorNode;
  while (n && n !== input) {
    if (n.nodeType === 1 && n.parentNode === input) return n;
    if (n.nodeType === 1 && n.tagName === 'LI') return n;
    n = n.parentNode;
  }
  return null;
}

function tryBlockMarkdown() {
  const selObj = window.getSelection();
  if (!selObj || !selObj.isCollapsed) return false;
  let blk = currentBlock();
  if (!blk) {
    const an = selObj.anchorNode;
    if (an && an.nodeType === 3 && an.parentNode === input) {
      const off = selObj.anchorOffset;
      const p = document.createElement('p');
      const tn = document.createTextNode(an.nodeValue);
      p.append(tn);
      an.replaceWith(p);
      const r = document.createRange();
      r.setStart(tn, Math.min(off, tn.nodeValue.length));
      r.collapse(true);
      selObj.removeAllRanges();
      selObj.addRange(r);
      blk = p;
    } else return false;
  }
  if (blk.tagName === 'PRE' || blk.tagName === 'LI') return false;
  const txt = blk.textContent.replace(/\u200B/g, '').replace(/\u00A0/g, ' ');
  let tag = null, rest = '';
  if (/^###\s/.test(txt)) { tag = 'h3'; rest = txt.replace(/^###\s/, ''); }
  else if (/^##\s/.test(txt)) { tag = 'h2'; rest = txt.replace(/^##\s/, ''); }
  else if (/^#\s/.test(txt)) { tag = 'h1'; rest = txt.replace(/^#\s/, ''); }
  else if (/^>\s/.test(txt)) { tag = 'blockquote'; rest = txt.replace(/^>\s/, ''); }
  else if (/^[-*]\s/.test(txt)) { tag = 'ul'; rest = txt.replace(/^[-*]\s/, ''); }
  else if (/^1\.\s/.test(txt)) { tag = 'ol'; rest = txt.replace(/^1\.\s/, ''); }
  else if (/^```/.test(txt)) { tag = 'pre'; rest = txt.replace(/^```\s?/, ''); }
  else if (/^---$/.test(txt.trim())) {
    const hr = document.createElement('hr');
    const p = document.createElement('p');
    p.append(document.createElement('br'));
    blk.replaceWith(hr);
    hr.after(p);
    placeCaret(p, false);
    return true;
  }
  if (!tag) return false;

  if (tag === 'ul' || tag === 'ol') {
    const list = document.createElement(tag);
    const li = document.createElement('li');
    li.textContent = rest;
    if (!rest) li.append(document.createElement('br'));
    list.append(li);
    blk.replaceWith(list);
    placeCaret(li, true);
  } else if (tag === 'pre') {
    const pre = document.createElement('pre');
    const code = document.createElement('code');
    code.textContent = rest;
    if (!rest) code.append(document.createElement('br'));
    pre.append(code);
    blk.replaceWith(pre);
    placeCaret(code, true);
  } else {
    const el = document.createElement(tag);
    el.textContent = rest;
    if (!rest) el.append(document.createElement('br'));
    blk.replaceWith(el);
    placeCaret(el, true);
  }
  return true;
}

function tryInlineMarkdown() {
  const selObj = window.getSelection();
  if (!selObj || !selObj.isCollapsed) return;
  const node = selObj.anchorNode;
  if (!node || node.nodeType !== 3) return;
  if (node.parentElement && node.parentElement.closest('pre, code')) return;
  const off = selObj.anchorOffset;
  const before = node.nodeValue.slice(0, off);
  const after = node.nodeValue.slice(off);
  for (const pat of [
    { re: /\*\*([^*\n]+)\*\*$/, tag: 'strong' },
    { re: /`([^`\n]+)`$/, tag: 'code' },
    { re: /(?:^|[^*])\*([^*\n]+)\*$/, tag: 'em', keepPrefix: true },
  ]) {
    const m = before.match(pat.re);
    if (!m) continue;
    let startIdx = m.index;
    if (pat.keepPrefix && m[0].length > m[1].length + 2) startIdx += 1;
    const lead = before.slice(0, startIdx);
    const frag = document.createDocumentFragment();
    if (lead) frag.append(document.createTextNode(lead));
    const el = document.createElement(pat.tag);
    el.textContent = m[1];
    frag.append(el);
    const tail = document.createTextNode('\u200B' + after);
    frag.append(tail);
    node.parentNode.replaceChild(frag, node);
    const r = document.createRange();
    r.setStart(tail, 1);
    r.collapse(true);
    selObj.removeAllRanges();
    selObj.addRange(r);
    return;
  }
}

function editorToMarkdown(root) {
  function inlineMd(node) {
    if (node.nodeType === 3) return node.nodeValue.replace(/\u200B/g, '');
    if (node.nodeType !== 1) return '';
    const tag = node.tagName;
    if (tag === 'BR') return '\n';
    const inner = [...node.childNodes].map(inlineMd).join('');
    if (tag === 'STRONG' || tag === 'B') return `**${inner}**`;
    if (tag === 'EM' || tag === 'I') return `*${inner}*`;
    if (tag === 'CODE') return `\`${inner}\``;
    return inner;
  }
  const lines = [];
  for (const child of root.childNodes) {
    if (child.nodeType === 3) {
      const t = child.nodeValue.replace(/\u200B/g, '');
      if (t) lines.push(t);
      continue;
    }
    if (child.nodeType !== 1) continue;
    const tag = child.tagName;
    if (tag === 'H1') lines.push(`# ${inlineMd(child).trim()}`);
    else if (tag === 'H2') lines.push(`## ${inlineMd(child).trim()}`);
    else if (tag === 'H3') lines.push(`### ${inlineMd(child).trim()}`);
    else if (tag === 'BLOCKQUOTE') lines.push(inlineMd(child).split('\n').map(l => `> ${l}`).join('\n'));
    else if (tag === 'PRE') lines.push(`\`\`\`\n${child.textContent.replace(/\u200B/g, '')}\n\`\`\``);
    else if (tag === 'HR') lines.push('---');
    else if (tag === 'UL') for (const li of child.querySelectorAll(':scope > li')) lines.push(`- ${inlineMd(li).trim()}`);
    else if (tag === 'OL') [...child.querySelectorAll(':scope > li')].forEach((li, i) => lines.push(`${i + 1}. ${inlineMd(li).trim()}`));
    else lines.push(inlineMd(child).replace(/\n$/, ''));
  }
  return lines.join('\n').replace(/\n{3,}/g, '\n\n').trim();
}

async function downscaleImage(dataUrl, maxDim = 640) {
  return new Promise(res => {
    const img = new Image();
    img.onload = () => {
      const { width, height } = img;
      if (width <= maxDim && height <= maxDim) {
        res(dataUrl);
        return;
      }
      const scale = maxDim / Math.max(width, height);
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(width * scale);
      canvas.height = Math.round(height * scale);
      const ctx = canvas.getContext('2d');
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      res(canvas.toDataURL('image/jpeg', 0.86));
    };
    img.onerror = () => res(dataUrl);
    img.src = dataUrl;
  });
}

function renderChips() {
  const box = $('#chips');
  box.innerHTML = '';
  state.attachments.forEach((a, i) => {
    const c = document.createElement('span');
    c.className = 'chip';
    if (a.kind === 'image' && a.previewUrl) {
      const img = document.createElement('img');
      img.className = 'chip-thumb';
      img.src = a.previewUrl;
      img.alt = a.name;
      c.append(img);
    }
    const label = document.createElement('span');
    label.textContent = a.name;
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '✕';
    del.onclick = () => {
      state.attachments.splice(i, 1);
      renderChips();
    };
    c.append(label, del);
    box.append(c);
  });
}

function setGenerating(active) {
  state.busy = active;
  $('#btn-send').disabled = active;
  $('#btn-send').hidden = active;
  $('#btn-stop').hidden = !active;
}

function buildApiMessages(msgs) {
  const out = [];
  for (let i = 0; i < msgs.length - 1; i++) {
    const cur = msgs[i];
    const next = msgs[i + 1];
    if (cur.role === 'user' && next && next.role === 'assistant' && next.cancelled) {
      i++;
      continue;
    }
    if (cur.cancelled || !String(cur.content || '').trim()) continue;
    out.push({
      role: cur.role,
      content: cur.content,
      images: cur.images || [],
    });
  }
  return out;
}

export function initChatEvents() {
  $('#chat-log')?.addEventListener('click', async ev => {
    const btn = ev.target.closest('.code .copy');
    if (!btn) return;
    const codeEl = btn.closest('.code')?.querySelector('pre code');
    if (!codeEl) return;
    try {
      await navigator.clipboard.writeText(codeEl.textContent || '');
      const prev = btn.textContent;
      btn.textContent = t('copied');
      btn.classList.add('copied');
      setTimeout(() => {
        btn.textContent = prev;
        btn.classList.remove('copied');
      }, 1600);
    } catch {}
  });

  input.addEventListener('keydown', e => {
    const blk = currentBlock();
    if (e.key === 'Backspace' && blk && ['H1', 'H2', 'H3', 'BLOCKQUOTE', 'PRE', 'LI'].includes(blk.tagName)) {
      if (!blk.textContent.replace(/\u200B/g, '')) {
        e.preventDefault();
        const p = document.createElement('p');
        p.append(document.createElement('br'));
        if (blk.tagName === 'LI') {
          const list = blk.parentNode;
          list.after(p);
          blk.remove();
          if (!list.children.length) list.remove();
        } else blk.replaceWith(p);
        placeCaret(p, false);
        return;
      }
    }
    if (e.key === 'Enter' && e.shiftKey) {
      if (blk && blk.tagName === 'PRE') return;
      e.preventDefault();
      const p = document.createElement('p');
      p.append(document.createElement('br'));
      if (blk && blk.tagName === 'LI') {
        const li = document.createElement('li');
        li.append(document.createElement('br'));
        blk.after(li);
        placeCaret(li, false);
        return;
      }
      if (blk) blk.after(p);
      else input.append(p);
      placeCaret(p, false);
      return;
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      if (blk && blk.tagName === 'PRE') return;
      e.preventDefault();
      $('#composer').requestSubmit();
    }
  });

  input.addEventListener('input', () => {
    if (!tryBlockMarkdown()) tryInlineMarkdown();
    if (!input.textContent.replace(/\u200B/g, '') && !input.querySelector('hr, pre, blockquote, ul, ol, h1, h2, h3')) {
      input.innerHTML = '';
    }
  });

  $('#btn-attach').onclick = () => $('#file-input').click();
  $('#file-input').onchange = async ev => {
    for (const f of ev.target.files) {
      if (f.type.startsWith('image/')) {
        const rawUrl = await new Promise(res => {
          const r = new FileReader();
          r.onload = () => res(r.result);
          r.readAsDataURL(f);
        });
        const optimizedUrl = await downscaleImage(String(rawUrl), 640);
        const b64 = String(optimizedUrl).split(',')[1] || '';
        state.attachments.push({ name: f.name, kind: 'image', data: b64, previewUrl: String(optimizedUrl) });
      } else {
        const txt = await f.text();
        state.attachments.push({ name: f.name, kind: 'text', data: txt });
      }
    }
    ev.target.value = '';
    renderChips();
  };

  $('#btn-new-chat').onclick = newConversation;

  $('#btn-export-md')?.addEventListener('click', async () => {
    const conv = activeConv();
    if (!conv || !conv.messages.length) return;
    const lines = [
      `# ${conv.title || 'Conversa Guanaco'}`,
      `> Exportado do **Guanaco Pro** em ${new Date().toLocaleString()}`,
      '',
    ];
    for (const m of conv.messages) {
      if (m.cancelled) continue;
      const who = m.role === 'user' ? `## 👤 ${t('you')}` : `## 🦙 ${t('guanaco')}`;
      lines.push(who, '');
      if (Array.isArray(m.sources) && m.sources.length) {
        lines.push(`*📚 ${t('sourcesConsulted')}: ${m.sources.join(', ')}*`, '');
      }
      lines.push(String(m.displayContent ?? m.content ?? '').trim(), '', '---', '');
    }
    try {
      await invoke('export_conversation_markdown', {
        title: conv.title || 'conversa-guanaco',
        markdown: lines.join('\n'),
      });
    } catch {}
  });

  $('#btn-stop').onclick = async () => {
    if (!state.busy) return;
    state.streamCancelled = true;
    stopThinkingTimer();
    try {
      await invoke('cancel_chat');
      const conv = state.conversations.find(c => c.id === state.streamingConvId) || activeConv();
      const last = conv?.messages?.[conv.messages.length - 1];
      if (last && last.role === 'assistant') {
        if (!String(last.content || '').trim()) {
          last.cancelled = true;
          last.content = t('cancelledMsg');
        }
        if (state.activeStreamBubble && state.activeConvId === conv.id) {
          state.activeStreamBubble.dataset.rawText = last.content;
          const body = state.activeStreamBubble.querySelector('.msg-body');
          if (last.cancelled) body.innerHTML = `<em class="muted">${t('cancelledMsg')}</em>`;
          else body.innerHTML = window.renderMarkdown(last.content);
        }
        await persistConversations();
      }
    } finally {
      setGenerating(false);
    }
  };

  $('#composer').onsubmit = async e => {
    e.preventDefault();
    if (state.busy) return;
    const model = $('#model-select').value;
    if (!model) {
      switchView('models');
      return;
    }
    const typedText = editorToMarkdown(input);
    if (!typedText && !state.attachments.length) return;

    const imgAtts = state.attachments.filter(a => a.kind === 'image');
    const imgs = imgAtts.map(a => a.data);
    const previews = imgAtts.map(a => a.previewUrl || `data:image/png;base64,${a.data}`);
    const textFiles = state.attachments.filter(a => a.kind === 'text');
    const fileNames = textFiles.map(f => f.name);

    let fullText = typedText;
    if (textFiles.length) {
      const extra = textFiles.map(f => `\n\n--- Arquivo: ${f.name} ---\n\`\`\`\n${f.data}\n\`\`\``).join('');
      fullText = (fullText + extra).trim();
    }
    const displayContent = typedText || (fileNames.length ? t('analyzeFilesPrompt') : '');

    state.attachments = [];
    renderChips();
    input.innerHTML = '';

    const conv = activeConv();
    if (!conv.messages.length) {
      const titleBase = typedText || fileNames[0] || t('newConvTitle');
      conv.title = titleBase.replace(/[#>*`-]/g, '').trim().slice(0, 38) || t('newConvTitle');
      renderConvList();
    }
    conv.messages.push({
      role: 'user',
      content: fullText,
      displayContent,
      images: imgs,
      fileNames,
    });
    if (conv.messages.length === 1) $('#chat-log').innerHTML = '';
    appendBubble('user', displayContent, previews, fileNames);

    const thinkingLabel = imgs.length ? t('analyzingImg') : (fileNames.length ? t('readingDoc') : t('thinking'));
    conv.messages.push({ role: 'assistant', content: '' });
    state.streamingConvId = conv.id;
    state.activeStreamBubble = appendBubble('assistant', '', [], [], thinkingLabel);
    $('#chat-log').scrollTop = $('#chat-log').scrollHeight;

    state.streamCancelled = false;
    setGenerating(true);
    startThinkingTimer(thinkingLabel);
    try {
      try {
        const ps = await invoke('list_running_models');
        const others = (ps.models || []).filter(r => r.name && r.name !== model);
        for (const o of others) {
          await invoke('unload_model', { model: o.name });
        }
      } catch {}

      const proActive = Boolean(state.license?.active);
      const proCfg = getProSettings();
      await invoke('chat_stream', {
        req: {
          convId: conv.id,
          model,
          ecoMode: $('#eco-mode-toggle')?.checked || false,
          profile: getResponseProfile(),
          customSystemPrompt: getEffectiveSystemPrompt(),
          temperature: proActive ? proCfg.temperature : null,
          topP: proActive ? proCfg.topP : null,
          numCtx: proActive ? proCfg.numCtx : null,
          repeatPenalty: proActive ? proCfg.repeatPenalty : null,
          numGpu: proActive ? proCfg.numGpu : null,
          knowledgeFolder: getEffectiveKnowledgeFolder(),
          messages: buildApiMessages(conv.messages),
        },
      });
    } catch (err) {
      if (!state.streamCancelled) {
        conv.messages[conv.messages.length - 1].content = `Erro: ${err}`;
        if (state.activeStreamBubble && state.activeConvId === conv.id) {
          state.activeStreamBubble.dataset.rawText = `Erro: ${err}`;
          state.activeStreamBubble.querySelector('.msg-body').textContent = `Erro: ${err}`;
        }
      }
    } finally {
      stopThinkingTimer();
      setGenerating(false);
      state.streamingConvId = null;
      state.thinkingLabel = null;
      state.thinkingStartedAt = 0;
      const last = conv.messages[conv.messages.length - 1];
      if (last && last.role === 'assistant' && !String(last.content || '').trim()) {
        last.cancelled = state.streamCancelled;
        last.content = state.streamCancelled ? t('cancelledMsg') : t('noReplyMsg');
        if (state.activeStreamBubble && state.activeConvId === conv.id) {
          state.activeStreamBubble.dataset.rawText = last.content;
          state.activeStreamBubble.querySelector('.msg-body').innerHTML = `<em class="muted">${last.content}</em>`;
        }
      }
      await persistConversations();
    }
  };
}
