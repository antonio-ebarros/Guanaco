const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = s => document.querySelector(s);
const $$ = s => [...document.querySelectorAll(s)];

const PRESETS = [
  { id: 'qwen2.5:1.5b', name: 'Rápido e Leve', sizeGb: 1.0, neededMemGb: 1.8, desc: 'Ideal para computadores básicos e respostas rápidas no dia a dia.' },
  { id: 'llama3.2:3b', name: 'Equilibrado', sizeGb: 2.0, neededMemGb: 2.8, desc: 'Ótimo equilíbrio entre velocidade, português claro e resumos.' },
  { id: 'gemma3:4b', name: 'Com Visão (Imagens)', sizeGb: 3.3, neededMemGb: 4.2, desc: 'Além de texto, entende fotos e prints que você anexar.' },
  { id: 'qwen2.5:7b', name: 'Mais Capaz', sizeGb: 4.7, neededMemGb: 5.8, desc: 'Melhor raciocínio e textos longos para PCs com mais memória.' },
];

let sysInfo = null;
let installed = [];
let running = [];
let conversations = [];
let activeConvId = null;
let attachments = [];
let busy = false;

function fmtGb(bytes) {
  return (bytes / 1073741824).toFixed(2) + ' GB';
}

function switchView(name) {
  $$('.nav-tab').forEach(b => b.classList.toggle('active', b.dataset.view === name));
  $$('.view').forEach(v => v.classList.toggle('active', v.id === `view-${name}`));
  if (name === 'control') refreshControl();
}

$$('.nav-tab').forEach(b => b.addEventListener('click', () => switchView(b.dataset.view)));

// ---- Conversas locais ----
async function loadConversations() {
  try {
    const raw = await invoke('load_conversations');
    conversations = JSON.parse(raw || '[]');
  } catch {
    conversations = [];
  }
  if (!conversations.length) newConversation();
  else {
    activeConvId = conversations[0].id;
    renderConvList();
    renderChat();
  }
}

async function persistConversations() {
  await invoke('save_conversations', { data: JSON.stringify(conversations) });
}

function newConversation() {
  const c = {
    id: String(Date.now()),
    title: 'Nova conversa',
    model: $('#model-select').value || installed[0]?.name || '',
    messages: [],
  };
  conversations.unshift(c);
  activeConvId = c.id;
  renderConvList();
  renderChat();
  switchView('chat');
}

function activeConv() {
  return conversations.find(c => c.id === activeConvId) || conversations[0];
}

function renderConvList() {
  const nav = $('#conv-list');
  nav.innerHTML = '';
  for (const c of conversations) {
    const btn = document.createElement('button');
    btn.className = 'conv-item' + (c.id === activeConvId ? ' active' : '');
    btn.type = 'button';
    btn.innerHTML = `<span class="conv-name"></span><span class="conv-del" title="Apagar">✕</span>`;
    btn.querySelector('.conv-name').textContent = c.title;
    btn.onclick = ev => {
      if (ev.target.classList.contains('conv-del')) {
        conversations = conversations.filter(x => x.id !== c.id);
        if (!conversations.length) newConversation();
        else if (activeConvId === c.id) activeConvId = conversations[0].id;
        persistConversations();
        renderConvList();
        renderChat();
        return;
      }
      activeConvId = c.id;
      renderConvList();
      renderChat();
      switchView('chat');
    };
    nav.append(btn);
  }
}

function renderChat() {
  const conv = activeConv();
  $('#chat-title').textContent = conv?.title || 'Nova conversa';
  const log = $('#chat-log');
  log.innerHTML = '';

  if (!sysInfo?.ollamaRunning) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = sysInfo?.ollamaInstalled
      ? `<h2>Motor de IA desligado</h2>
         <p>O motor local (Ollama) está instalado no seu computador, mas está parado no momento.</p>
         <button id="wb-start" class="btn-primary" type="button">Ligar Motor de IA agora</button>`
      : `<h2>Bem-vindo ao Guanaco</h2>
         <p>Para rodar modelos de IA offline no seu PC, instale o motor gratuito <b>Ollama</b> (ollama.com) e clique em atualizar.</p>
         <button id="wb-recheck" class="btn-ghost" type="button">Verificar novamente</button>`;
    log.append(box);
    box.querySelector('#wb-start')?.addEventListener('click', startEngine);
    box.querySelector('#wb-recheck')?.addEventListener('click', bootstrap);
    return;
  }

  if (!installed.length) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = `
      <h2>Escolha seu primeiro modelo de IA</h2>
      <p>Seu computador tem <b>${sysInfo.totalRamGb} GB de RAM</b>. Escolha uma opção abaixo para baixar e começar a conversar — sem terminal:</p>
      <button id="wb-go-models" class="btn-primary" type="button">Escolher e baixar modelo</button>
    `;
    log.append(box);
    box.querySelector('#wb-go-models').onclick = () => switchView('models');
    return;
  }

  if (!conv || !conv.messages.length) {
    const box = document.createElement('div');
    box.className = 'welcome-box';
    box.innerHTML = `
      <h2>Tudo pronto</h2>
      <p>Você está usando <b>${$('#model-select').value || 'modelo local'}</b>. Tudo o que você digitar fica apenas neste computador.</p>
    `;
    log.append(box);
    return;
  }

  for (const m of conv.messages) {
    appendBubble(m.role, m.displayContent ?? m.content, m.previews || m.images || [], m.fileNames || []);
  }
  log.scrollTop = log.scrollHeight;
}

function renderThinkingHtml(label = 'Pensando', sec = 0) {
  return `<div class="thinking-indicator"><span class="thinking-dots"><span></span><span></span><span></span></span><span>${label}… ${sec}s</span></div>`;
}

function appendBubble(role, content, media = [], fileNames = [], thinkingLabel = null) {
  const el = document.createElement('div');
  el.className = `msg ${role}`;
  el.innerHTML = `<div class="msg-role">${role === 'user' ? 'Você' : 'Guanaco'}</div><div class="msg-thumbs" hidden></div><div class="msg-files" hidden></div><div class="msg-body"></div>`;
  const thumbs = el.querySelector('.msg-thumbs');
  if (Array.isArray(media) && media.length) {
    thumbs.hidden = false;
    for (const item of media) {
      const img = document.createElement('img');
      img.className = 'msg-thumb';
      img.alt = 'Imagem anexada';
      img.src = item.startsWith('data:') ? item : `data:image/png;base64,${item}`;
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
  } else if (typeof renderMarkdown === 'function') {
    body.innerHTML = renderMarkdown(content || '');
  } else {
    body.textContent = content || '';
  }
  $('#chat-log').append(el);
  return el;
}

// ---- Hardware, Modelos & Controle ----
async function bootstrap() {
  sysInfo = await invoke('get_system_info');
  const dot = $('#engine-dot');
  const lbl = $('#engine-label');
  const mem = $('#engine-mem');
  const btnStart = $('#btn-start-engine');

  mem.textContent = `${sysInfo.totalRamGb} GB RAM • ${sysInfo.cpuCores} núcleos`;
  if (sysInfo.ollamaRunning) {
    dot.className = 'dot ok';
    lbl.textContent = 'Motor local ativo';
    btnStart.hidden = true;
    await refreshModels();
  } else if (sysInfo.ollamaInstalled) {
    dot.className = 'dot err';
    lbl.textContent = 'Motor desligado';
    btnStart.hidden = false;
    installed = [];
  } else {
    dot.className = 'dot err';
    lbl.textContent = 'Ollama não instalado';
    btnStart.hidden = true;
    installed = [];
  }
  renderModelsView();
  renderChat();
}

async function startEngine() {
  $('#engine-label').textContent = 'Ligando motor…';
  try {
    await invoke('start_ollama');
    await bootstrap();
  } catch (e) {
    alert(String(e));
    await bootstrap();
  }
}

$('#btn-start-engine').onclick = startEngine;

async function refreshModels() {
  try {
    const res = await invoke('list_installed_models');
    installed = res.models || [];
  } catch {
    installed = [];
  }
  try {
    const ps = await invoke('list_running_models');
    running = ps.models || [];
  } catch {
    running = [];
  }
  const sel = $('#model-select');
  const prev = sel.value;
  sel.innerHTML = '';
  for (const m of installed) {
    const opt = document.createElement('option');
    opt.value = m.name;
    opt.textContent = `${m.name} (${fmtGb(m.size || 0)})`;
    sel.append(opt);
  }
  if (prev && installed.some(m => m.name === prev)) sel.value = prev;
  renderModelsView();
}

function evaluatePresetFit(p) {
  if (!sysInfo) return { cls: 'badge-ok', label: 'Verificando PC…' };
  const osReserve = sysInfo.osReservedGb ?? 1.5;
  const usableRamNow = Math.max(0, (sysInfo.freeRamGb || 0) - osReserve);
  const hasGpu = Boolean(sysInfo.gpuName);
  const usableVramNow = hasGpu ? Math.max(0, (sysInfo.gpuVramFreeGb || 0) - 0.3) : 0;
  const usableVramTotal = hasGpu ? Math.max(0, (sysInfo.gpuVramTotalGb || 0) - 0.3) : 0;
  const budgetNow = usableRamNow + usableVramNow;
  const runningMemGb = running.reduce((acc, m) => acc + (m.size || 0) / 1073741824, 0);
  const budgetAfterUnload = budgetNow + runningMemGb;
  const maxHardwareBudget = Math.max(0, (sysInfo.totalRamGb || 0) - osReserve) + usableVramTotal;
  const need = p.neededMemGb;

  if (usableVramNow >= need) {
    return { cls: 'badge-ok', label: 'Roda rápido na GPU agora' };
  }
  if (budgetNow >= need) {
    if (!hasGpu && (sysInfo.cpuCores || 0) <= 2 && need > 2.5) {
      return { cls: 'badge-warn', label: 'Cabe na RAM, mas CPU lenta' };
    }
    return { cls: 'badge-ok', label: hasGpu ? 'Roda bem agora (GPU + RAM)' : 'Roda bem na RAM agora' };
  }
  if (running.length && budgetAfterUnload >= need) {
    return { cls: 'badge-warn', label: 'Libere modelos ativos para rodar' };
  }
  if (maxHardwareBudget >= need) {
    return { cls: 'badge-warn', label: `Pouca memória livre agora (~${need} GB)` };
  }
  return { cls: 'badge-err', label: `Pesado demais (~${need} GB em uso)` };
}

function renderModelsView() {
  if (sysInfo) {
    const osReserve = sysInfo.osReservedGb ?? 1.5;
    const runningMemGb = running.reduce((acc, m) => acc + (m.size || 0) / 1073741824, 0);
    const gpuText = sysInfo.gpuName
      ? `GPU: <b>${sysInfo.gpuName}</b> (${sysInfo.gpuVramFreeGb} GB livres de ${sysInfo.gpuVramTotalGb} GB VRAM)`
      : `Sem GPU NVIDIA dedicada (rodará na CPU)`;
    const runningHtml = running.length
      ? `<div class="row-between" style="margin-top:4px;">
           <span class="muted">Modelos carregados agora: <b>${running.length}</b> ocupando <b>${runningMemGb.toFixed(2)} GB</b></span>
           <button id="btn-unload-all-models" class="btn-ghost small" type="button">Liberar memória</button>
         </div>`
      : `<div class="muted">Nenhum modelo carregado na memória no momento.</div>`;

    $('#hw-banner').innerHTML = `
      <div><strong>Memória livre agora:</strong> <b>${sysInfo.freeRamGb} GB</b> livres de ${sysInfo.totalRamGb} GB totais (reservando ~${osReserve} GB para o sistema)</div>
      <div class="muted">CPU: ${sysInfo.cpuName} (${sysInfo.cpuCores} núcleos) • ${gpuText}</div>
      ${runningHtml}
    `;
    $('#btn-unload-all-models')?.addEventListener('click', async () => {
      for (const r of running) {
        await invoke('unload_model', { model: r.name });
      }
      await bootstrap();
    });
  }

  const grid = $('#preset-grid');
  grid.innerHTML = '';
  for (const p of PRESETS) {
    const fit = evaluatePresetFit(p);
    const has = installed.some(m => m.name === p.id);
    const card = document.createElement('div');
    card.className = 'info-card';
    card.innerHTML = `
      <div class="row-between">
        <strong>${p.name}</strong>
        <span class="muted">${p.sizeGb} GB disco • ~${p.neededMemGb} GB uso</span>
      </div>
      <span class="${fit.cls}">${fit.label}</span>
      <p class="muted">${p.desc}</p>
      <div class="row-between">
        <code class="muted">${p.id}</code>
        <button class="${has ? 'btn-ghost small' : 'btn-primary'}" type="button">${has ? 'Usar' : 'Baixar'}</button>
      </div>
    `;
    card.querySelector('button').onclick = () => {
      if (has) {
        $('#model-select').value = p.id;
        switchView('chat');
      } else {
        pullModel(p.id);
      }
    };
    grid.append(card);
  }

  const list = $('#installed-list');
  list.innerHTML = '';
  if (!installed.length) {
    list.innerHTML = `<div class="list-row"><span class="muted">Nenhum modelo baixado ainda.</span></div>`;
  } else {
    for (const m of installed) {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `
        <div>
          <strong>${m.name}</strong>
          <div class="muted">${fmtGb(m.size || 0)} em disco</div>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn-ghost small use-btn" type="button">Usar</button>
          <button class="btn-ghost small danger del-btn" type="button">Apagar</button>
        </div>
      `;
      row.querySelector('.use-btn').onclick = () => {
        $('#model-select').value = m.name;
        switchView('chat');
      };
      row.querySelector('.del-btn').onclick = async () => {
        if (!confirm(`Remover ${m.name} e liberar ${fmtGb(m.size || 0)}?`)) return;
        await invoke('delete_model', { model: m.name });
        await refreshModels();
        renderChat();
      };
      list.append(row);
    }
  }
}

async function pullModel(name) {
  if (!sysInfo?.ollamaRunning) {
    alert('Ligue o Motor de IA antes de baixar um modelo.');
    return;
  }
  $('#pull-card').hidden = false;
  $('#pull-title').textContent = `Baixando ${name}…`;
  $('#pull-pct').textContent = '0%';
  $('#pull-fill').style.width = '0%';
  try {
    await invoke('pull_model', { model: name });
    $('#pull-card').hidden = true;
    await refreshModels();
    $('#model-select').value = name;
    renderChat();
  } catch (e) {
    $('#pull-status').textContent = `Erro: ${e}`;
  }
}

$('#custom-pull-form').onsubmit = e => {
  e.preventDefault();
  const name = $('#custom-model-name').value.trim();
  if (name) pullModel(name);
};

$('#btn-refresh-models').onclick = bootstrap;
$('#btn-refresh-control').onclick = refreshControl;

async function refreshControl() {
  await bootstrap();
  $('#ctrl-hw').textContent = `${sysInfo.freeRamGb} GB livres de ${sysInfo.totalRamGb} GB RAM`;
  $('#ctrl-gpu').textContent = sysInfo.gpuName
    ? `CPU: ${sysInfo.cpuName} (${sysInfo.cpuCores} núcleos) • GPU: ${sysInfo.gpuName} (${sysInfo.gpuVramFreeGb}/${sysInfo.gpuVramTotalGb} GB VRAM livres)`
    : `CPU: ${sysInfo.cpuName} (${sysInfo.cpuCores} núcleos) • Sem GPU dedicada`;

  const runBox = $('#running-list');
  runBox.innerHTML = '';
  try {
    const ps = await invoke('list_running_models');
    running = ps.models || [];
  } catch {
    running = [];
  }
  if (!running.length) {
    runBox.innerHTML = `<div class="list-row"><span class="muted">Nenhum modelo carregado na memória no momento.</span></div>`;
  } else {
    for (const r of running) {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `
        <div>
          <strong>${r.name}</strong>
          <div class="muted">Ocupando ${fmtGb(r.size || 0)} na memória</div>
        </div>
        <button class="btn-ghost small" type="button">Liberar Memória</button>
      `;
      row.querySelector('button').onclick = async () => {
        await invoke('unload_model', { model: r.name });
        await refreshControl();
      };
      runBox.append(row);
    }
  }

  const totalDisk = installed.reduce((acc, m) => acc + (m.size || 0), 0);
  $('#storage-card').innerHTML = `
    <div><strong>Pasta dos modelos:</strong> <code>${sysInfo.modelsDir}</code></div>
    <div class="muted">${installed.length} modelo(s) salvo(s) ocupando <b>${fmtGb(totalDisk)}</b> no disco.</div>
  `;
}

// ---- Editor Markdown ao vivo ----
const input = $('#input');

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

// ---- Anexos e Envio ----
$('#btn-attach').onclick = () => $('#file-input').click();
$('#file-input').onchange = async ev => {
  for (const f of ev.target.files) {
    if (f.type.startsWith('image/')) {
      const dataUrl = await new Promise(res => {
        const r = new FileReader();
        r.onload = () => res(r.result);
        r.readAsDataURL(f);
      });
      const b64 = String(dataUrl).split(',')[1] || '';
      attachments.push({ name: f.name, kind: 'image', data: b64, previewUrl: String(dataUrl) });
    } else {
      const txt = await f.text();
      attachments.push({ name: f.name, kind: 'text', data: txt });
    }
  }
  ev.target.value = '';
  renderChips();
};

function renderChips() {
  const box = $('#chips');
  box.innerHTML = '';
  attachments.forEach((a, i) => {
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
      attachments.splice(i, 1);
      renderChips();
    };
    c.append(label, del);
    box.append(c);
  });
}

$('#btn-new-chat').onclick = newConversation;

let activeStreamBubble = null;
let thinkingTimer = null;
let streamCancelled = false;

function stopThinkingTimer() {
  if (thinkingTimer) {
    clearInterval(thinkingTimer);
    thinkingTimer = null;
  }
}

function startThinkingTimer(label = 'Pensando') {
  stopThinkingTimer();
  const startedAt = Date.now();
  thinkingTimer = setInterval(() => {
    if (!activeStreamBubble) return;
    const sec = Math.floor((Date.now() - startedAt) / 1000);
    const body = activeStreamBubble.querySelector('.msg-body');
    if (body && body.querySelector('.thinking-indicator')) {
      body.innerHTML = renderThinkingHtml(label, sec);
    }
  }, 1000);
}

function setGenerating(active) {
  busy = active;
  $('#btn-send').disabled = active;
  $('#btn-send').hidden = active;
  $('#btn-stop').hidden = !active;
}

$('#btn-stop').onclick = async () => {
  if (!busy) return;
  streamCancelled = true;
  stopThinkingTimer();
  try {
    await invoke('cancel_chat');
    const conv = activeConv();
    const last = conv?.messages?.[conv.messages.length - 1];
    if (last && last.role === 'assistant') {
      if (!String(last.content || '').trim()) {
        last.content = '*Geração cancelada.*';
      }
      if (activeStreamBubble) {
        activeStreamBubble.querySelector('.msg-body').innerHTML = renderMarkdown(last.content);
      }
      await persistConversations();
    }
  } finally {
    setGenerating(false);
  }
};

$('#composer').onsubmit = async e => {
  e.preventDefault();
  if (busy) return;
  const model = $('#model-select').value;
  if (!model) {
    switchView('models');
    return;
  }
  const typedText = editorToMarkdown(input);
  if (!typedText && !attachments.length) return;

  const imgAtts = attachments.filter(a => a.kind === 'image');
  const imgs = imgAtts.map(a => a.data);
  const previews = imgAtts.map(a => a.previewUrl || `data:image/png;base64,${a.data}`);
  const textFiles = attachments.filter(a => a.kind === 'text');
  const fileNames = textFiles.map(f => f.name);

  let fullText = typedText;
  if (textFiles.length) {
    const extra = textFiles.map(f => `\n\n--- Arquivo: ${f.name} ---\n\`\`\`\n${f.data}\n\`\`\``).join('');
    fullText = (fullText + extra).trim();
  }
  const displayContent = typedText || (fileNames.length ? 'Analise os arquivos anexados:' : '');

  attachments = [];
  renderChips();
  input.innerHTML = '';

  const conv = activeConv();
  if (conv.title === 'Nova conversa') {
    const titleBase = typedText || fileNames[0] || 'Conversa';
    conv.title = titleBase.replace(/[#>*`-]/g, '').trim().slice(0, 38) || 'Conversa';
    renderConvList();
  }
  conv.messages.push({
    role: 'user',
    content: fullText,
    displayContent,
    images: imgs,
    previews,
    fileNames,
  });
  if (conv.messages.length === 1) $('#chat-log').innerHTML = '';
  appendBubble('user', displayContent, previews, fileNames);

  const thinkingLabel = imgs.length ? 'Analisando imagem' : (fileNames.length ? 'Lendo documento' : 'Pensando');
  conv.messages.push({ role: 'assistant', content: '' });
  activeStreamBubble = appendBubble('assistant', '', [], [], thinkingLabel);
  $('#chat-log').scrollTop = $('#chat-log').scrollHeight;

  streamCancelled = false;
  setGenerating(true);
  startThinkingTimer(thinkingLabel);
  try {
    // Descarrega outros modelos carregados para não acumular 2+ modelos na RAM/VRAM
    try {
      const ps = await invoke('list_running_models');
      const others = (ps.models || []).filter(r => r.name && r.name !== model);
      for (const o of others) {
        await invoke('unload_model', { model: o.name });
      }
    } catch {}

    await invoke('chat_stream', {
      req: {
        convId: conv.id,
        model,
        messages: conv.messages.slice(0, -1).map(m => ({
          role: m.role,
          content: m.content,
          images: m.images || [],
        })),
      },
    });
  } catch (err) {
    if (!streamCancelled) {
      conv.messages[conv.messages.length - 1].content = `Erro: ${err}`;
      activeStreamBubble.querySelector('.msg-body').textContent = `Erro: ${err}`;
    }
  } finally {
    stopThinkingTimer();
    setGenerating(false);
    const last = conv.messages[conv.messages.length - 1];
    if (last && last.role === 'assistant' && !String(last.content || '').trim()) {
      last.content = streamCancelled ? '*Geração cancelada.*' : '*Sem resposta do modelo.*';
      if (activeStreamBubble) {
        activeStreamBubble.querySelector('.msg-body').innerHTML = renderMarkdown(last.content);
      }
    }
    await persistConversations();
  }
};

// ---- Eventos Tauri ----
listen('pull-progress', ev => {
  const p = ev.payload;
  $('#pull-status').textContent = p.status || 'Baixando…';
  if (p.total > 0) {
    const pct = Math.round(p.percent);
    $('#pull-pct').textContent = `${pct}%`;
    $('#pull-fill').style.width = `${pct}%`;
  }
});

listen('chat-event', ev => {
  if (streamCancelled) return;
  const { convId, delta, done, error } = ev.payload;
  const conv = conversations.find(c => c.id === convId);
  if (!conv) return;
  const last = conv.messages[conv.messages.length - 1];
  if (!last || last.role !== 'assistant') return;
  if (error) last.content += `\n\nErro: ${error}`;
  else if (delta) last.content += delta;

  if (convId === activeConvId && activeStreamBubble) {
    const body = activeStreamBubble.querySelector('.msg-body');
    if (String(last.content || '').trim()) {
      stopThinkingTimer();
      if (typeof renderMarkdown === 'function') body.innerHTML = renderMarkdown(last.content);
      else body.textContent = last.content;
    }
    $('#chat-log').scrollTop = $('#chat-log').scrollHeight;
  }
  if (done) {
    stopThinkingTimer();
    persistConversations();
  }
});

// ---- Tema & Tamanho da Fonte ----
function applyTheme(theme) {
  const t = theme === 'dark' ? 'dark' : 'light';
  document.documentElement.dataset.theme = t;
  localStorage.setItem('guanaco-theme', t);
  const btn = $('#btn-theme');
  if (btn) btn.textContent = t === 'dark' ? '☀️ Claro' : '🌙 Escuro';
}

function applyFontSize(px) {
  const size = Math.min(26, Math.max(14, Number(px) || 18));
  document.documentElement.style.setProperty('--chat-font-size', `${size}px`);
  localStorage.setItem('guanaco-font-size', String(size));
  const lbl = $('#font-size-label');
  if (lbl) lbl.textContent = `${size}px`;
}

$('#btn-theme')?.addEventListener('click', () => {
  const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
  applyTheme(next);
});

$('#btn-font-dec')?.addEventListener('click', () => {
  const cur = Number(localStorage.getItem('guanaco-font-size') || 18);
  applyFontSize(cur - 2);
});

$('#btn-font-inc')?.addEventListener('click', () => {
  const cur = Number(localStorage.getItem('guanaco-font-size') || 18);
  applyFontSize(cur + 2);
});

applyTheme(localStorage.getItem('guanaco-theme') || 'light');
applyFontSize(localStorage.getItem('guanaco-font-size') || 18);

bootstrap().then(loadConversations);
