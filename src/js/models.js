import { $, PRESETS, fmtGb, invoke, state, switchView } from './state.js';
import { getLang, t } from './i18n.js';
import { getResponseProfile, setResponseProfile } from './theme.js';
import { renderProControlSection, renderProModelsBuilder } from './pro.js';

let onRenderChat = () => {};

export function setChatRenderer(fn) {
  onRenderChat = fn;
}

export function renderEngineStatus() {
  const s = state.sysInfo;
  if (!s) return;
  const dot = $('#engine-dot');
  const lbl = $('#engine-label');
  const mem = $('#engine-mem');
  const btnStart = $('#btn-start-engine');

  mem.textContent = t('coresLabel', { ram: s.totalRamGb, cores: s.cpuCores });
  if (s.ollamaRunning) {
    dot.className = 'dot ok';
    lbl.textContent = t('engineActive');
    btnStart.hidden = true;
  } else if (s.ollamaInstalled) {
    dot.className = 'dot err';
    lbl.textContent = t('engineOff');
    btnStart.hidden = false;
  } else {
    dot.className = 'dot err';
    lbl.textContent = t('engineNotReady');
    btnStart.hidden = true;
  }
}

export async function bootstrap() {
  state.sysInfo = await invoke('get_system_info');
  renderEngineStatus();
  if (state.sysInfo.ollamaRunning) {
    await refreshModels();
  } else {
    state.installed = [];
  }
  renderModelsView();
  if (!state.busy) onRenderChat();
}

export async function startEngine() {
  $('#engine-label').textContent = t('startingEngine');
  try {
    await invoke('start_ollama');
    await bootstrap();
  } catch (e) {
    alert(String(e));
    await bootstrap();
  }
}

export async function installEngine() {
  const btn = $('#wb-install');
  const box = $('#eng-install-box');
  if (btn) btn.disabled = true;
  if (box) box.hidden = false;
  try {
    await invoke('install_ollama');
    await bootstrap();
  } catch (e) {
    const st = $('#eng-install-status');
    if (st) st.textContent = `${t('errPrefix')}: ${e}`;
    if (btn) btn.disabled = false;
  }
}

export async function refreshModels() {
  try {
    const res = await invoke('list_installed_models');
    state.installed = res.models || [];
  } catch {
    state.installed = [];
  }
  try {
    const ps = await invoke('list_running_models');
    state.running = ps.models || [];
  } catch {
    state.running = [];
  }
  const sel = $('#model-select');
  const prev = sel.value;
  sel.innerHTML = '';
  for (const m of state.installed) {
    const opt = document.createElement('option');
    opt.value = m.name;
    opt.textContent = `${m.name} (${fmtGb(m.size || 0)})`;
    sel.append(opt);
  }
  if (prev && state.installed.some(m => m.name === prev)) sel.value = prev;
  renderModelsView();
}

export function evaluatePresetFit(p) {
  const s = state.sysInfo;
  if (!s) return { cls: 'badge-ok', label: t('fitChecking') };
  const osReserve = s.osReservedGb ?? 1.5;
  const usableRamNow = Math.max(0, (s.freeRamGb || 0) - osReserve);
  const hasGpu = Boolean(s.gpuName);
  const usableVramNow = hasGpu ? Math.max(0, (s.gpuVramFreeGb || 0) - 0.3) : 0;
  const usableVramTotal = hasGpu ? Math.max(0, (s.gpuVramTotalGb || 0) - 0.3) : 0;
  const budgetNow = usableRamNow + usableVramNow;
  const runningMemGb = state.running.reduce((acc, m) => acc + (m.size || 0) / 1073741824, 0);
  const budgetAfterUnload = budgetNow + runningMemGb;
  const maxHardwareBudget = Math.max(0, (s.totalRamGb || 0) - osReserve) + usableVramTotal;
  const need = p.neededMemGb;

  if (usableVramNow >= need) {
    return { cls: 'badge-ok', label: t('fitGpuFast') };
  }
  if (budgetNow >= need) {
    if (!hasGpu && (s.cpuCores || 0) <= 2 && need > 2.5) {
      return { cls: 'badge-warn', label: t('fitCpuSlow') };
    }
    return { cls: 'badge-ok', label: hasGpu ? t('fitGoodGpuRam') : t('fitGoodRam') };
  }
  if (state.running.length && budgetAfterUnload >= need) {
    return { cls: 'badge-warn', label: t('fitUnloadFirst') };
  }
  if (maxHardwareBudget >= need) {
    return { cls: 'badge-warn', label: t('fitLowFreeMem', { need }) };
  }
  return { cls: 'badge-err', label: t('fitTooHeavy', { need }) };
}

export function renderModelsView() {
  const s = state.sysInfo;
  if (s) {
    const osReserve = s.osReservedGb ?? 1.5;
    const runningMemGb = state.running.reduce((acc, m) => acc + (m.size || 0) / 1073741824, 0);
    const gpuText = s.gpuName
      ? t('gpuAvailable', { name: s.gpuName, free: s.gpuVramFreeGb, total: s.gpuVramTotalGb })
      : t('noDedicatedGpu');
    const runningHtml = state.running.length
      ? `<div class="row-between" style="margin-top:4px;">
           <span class="muted">${t('loadedModelsNow', { count: state.running.length, gb: runningMemGb.toFixed(2) })}</span>
           <button id="btn-unload-all-models" class="btn-ghost small" type="button">${t('freeMemBtn')}</button>
         </div>`
      : `<div class="muted">${t('noLoadedModels')}</div>`;

    $('#hw-banner').innerHTML = `
      <div>${t('memFreeBanner', { free: s.freeRamGb, total: s.totalRamGb, reserve: osReserve })}</div>
      <div class="muted">${t('cpuGpuLine', { cpu: s.cpuName, cores: s.cpuCores, gpu: gpuText })}</div>
      ${runningHtml}
    `;
    $('#btn-unload-all-models')?.addEventListener('click', async () => {
      for (const r of state.running) {
        await invoke('unload_model', { model: r.name });
      }
      await bootstrap();
    });
  }

  const grid = $('#preset-grid');
  grid.innerHTML = '';
  for (const p of PRESETS) {
    const fit = evaluatePresetFit(p);
    const has = state.installed.some(m => m.name === p.id);
    const card = document.createElement('div');
    card.className = 'info-card';
    card.innerHTML = `
      <div class="row-between">
        <strong>${t(p.nameKey)}</strong>
        <span class="muted">${t('diskAndUsage', { disk: p.sizeGb, need: p.neededMemGb })}</span>
      </div>
      <span class="${fit.cls}">${fit.label}</span>
      <p class="muted">${t(p.descKey)}</p>
      <div class="row-between">
        <code class="muted">${p.id}</code>
        <button class="${has ? 'btn-ghost small' : 'btn-primary'}" type="button">${has ? t('use') : t('download')}</button>
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
  if (!state.installed.length) {
    list.innerHTML = `<div class="list-row"><span class="muted">${t('noInstalledModels')}</span></div>`;
  } else {
    for (const m of state.installed) {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `
        <div>
          <strong>${m.name}</strong>
          <div class="muted">${t('onDisk', { size: fmtGb(m.size || 0) })}</div>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn-ghost small use-btn" type="button">${t('use')}</button>
          <button class="btn-ghost small danger del-btn" type="button">${t('delete')}</button>
        </div>
      `;
      row.querySelector('.use-btn').onclick = () => {
        $('#model-select').value = m.name;
        switchView('chat');
      };
      row.querySelector('.del-btn').onclick = async () => {
        if (!confirm(t('confirmDelete', { name: m.name, size: fmtGb(m.size || 0) }))) return;
        await invoke('delete_model', { model: m.name });
        await refreshModels();
        if (!state.busy) onRenderChat();
      };
      list.append(row);
    }
  }
  renderProModelsBuilder(async newName => {
    await refreshModels();
    if (newName) $('#model-select').value = newName;
    if (!state.busy) onRenderChat();
  });
}

export async function pullModel(name) {
  if (!state.sysInfo?.ollamaRunning) {
    alert(t('startBeforePull'));
    return;
  }
  $('#pull-card').hidden = false;
  $('#pull-title').textContent = `${t('downloadingModel')} (${name})`;
  $('#pull-pct').textContent = '0%';
  $('#pull-fill').style.width = '0%';
  try {
    await invoke('pull_model', { model: name });
    $('#pull-card').hidden = true;
    await refreshModels();
    $('#model-select').value = name;
    if (!state.busy) onRenderChat();
  } catch (e) {
    $('#pull-status').textContent = `${t('errPrefix')}: ${e}`;
  }
}

export function renderProfileGrid() {
  const grid = $('#profile-grid');
  if (!grid) return;
  const isPro = Boolean(state.license?.active);
  const active = getResponseProfile();
  const profiles = [
    { id: 'conservative', nameKey: 'profConsName', tagKey: 'profConsTag', descKey: 'profConsDesc', locked: false },
    { id: 'balanced', nameKey: 'profBalName', tagKey: 'profBalTag', descKey: 'profBalDesc', locked: false },
    { id: 'free', nameKey: 'profFreeName', tagKey: 'profFreeTag', descKey: 'profFreeDesc', locked: !isPro },
  ];
  grid.innerHTML = '';
  for (const p of profiles) {
    const isActive = !p.locked && active === p.id;
    const card = document.createElement('div');
    card.className = 'info-card profile-card' + (isActive ? ' active' : '') + (p.locked ? ' locked' : '');
    const badgeCls = p.locked ? 'badge-pro' : (isActive ? 'badge-ok' : 'badge-pro');
    const btnHtml = p.locked
      ? `<button class="btn-ghost small" type="button">${t('profLockedBtn')}</button>`
      : `<button class="${isActive ? 'btn-primary' : 'btn-ghost small'}" type="button">${isActive ? t('profActive') : t('profSelect')}</button>`;
    card.innerHTML = `
      <div class="row-between">
        <strong>${t(p.nameKey)}</strong>
        <span class="${badgeCls}">${t(p.tagKey)}</span>
      </div>
      <p class="muted">${t(p.descKey)}</p>
      <div class="row-between"><span></span>${btnHtml}</div>
    `;
    if (!p.locked) {
      card.querySelector('button').onclick = () => {
        setResponseProfile(p.id);
        renderProfileGrid();
      };
    } else {
      card.querySelector('button').onclick = () => {
        $('#pro-license-card')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        setTimeout(() => $('#pro-key-input')?.focus(), 250);
      };
    }
    grid.append(card);
  }
}

export function renderControlView() {
  renderProfileGrid();
  renderProControlSection(() => {
    renderControlView();
    renderModelsView();
  });
  const s = state.sysInfo;
  if (!s) return;
  $('#ctrl-hw').textContent = t('ctrlHwSummary', { free: s.freeRamGb, total: s.totalRamGb });
  $('#ctrl-gpu').textContent = s.gpuName
    ? t('ctrlGpuLine', { cpu: s.cpuName, cores: s.cpuCores, name: s.gpuName, free: s.gpuVramFreeGb, total: s.gpuVramTotalGb })
    : t('ctrlCpuOnlyLine', { cpu: s.cpuName, cores: s.cpuCores });

  const runBox = $('#running-list');
  runBox.innerHTML = '';
  if (!state.running.length) {
    runBox.innerHTML = `<div class="list-row"><span class="muted">${t('noLoadedModels')}</span></div>`;
  } else {
    for (const r of state.running) {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `
        <div>
          <strong>${r.name}</strong>
          <div class="muted">${t('usingInMem', { size: fmtGb(r.size || 0) })}</div>
        </div>
        <button class="btn-ghost small" type="button">${t('freeMemTitleBtn')}</button>
      `;
      row.querySelector('button').onclick = async () => {
        await invoke('unload_model', { model: r.name });
        await refreshControl();
      };
      runBox.append(row);
    }
  }

  const totalDisk = state.installed.reduce((acc, m) => acc + (m.size || 0), 0);
  $('#storage-card').innerHTML = `
    <div><strong>${t('modelsFolder')}</strong> <code>${s.modelsDir}</code></div>
    <div class="muted">${t('diskSummary', { count: state.installed.length, size: fmtGb(totalDisk) })}</div>
  `;

  renderTtsCard();
}

export async function renderTtsCard() {
  const box = $('#tts-card');
  if (!box) return;
  try {
    const lang = getLang();
    const st = await invoke('get_tts_status', { lang });
    if (st.installed) {
      box.innerHTML = `
        <div>${t('voiceReady')}</div>
        <div class="muted"><code>${st.dir}</code></div>
      `;
    } else {
      box.innerHTML = `
        <p class="muted">${t('voiceNotInstalled')}</p>
        <div id="tts-prog-box" hidden style="display:flex;flex-direction:column;gap:6px;">
          <div class="row-between">
            <span id="tts-prog-status" class="muted">${t('downloadingStatus')}</span>
            <span id="tts-prog-pct">0%</span>
          </div>
          <div class="progress-track"><div id="tts-prog-fill" class="progress-fill"></div></div>
        </div>
        <div class="row-between">
          <button id="btn-tts-install" class="btn-primary" type="button">${t('voiceDownloadBtn')}</button>
        </div>
      `;
      $('#btn-tts-install')?.addEventListener('click', async () => {
        const btn = $('#btn-tts-install');
        const prog = $('#tts-prog-box');
        if (btn) btn.disabled = true;
        if (prog) prog.hidden = false;
        try {
          await invoke('install_piper_tts', { lang });
          await renderTtsCard();
        } catch (e) {
          const ps = $('#tts-prog-status');
          if (ps) ps.textContent = `${t('errPrefix')}: ${e}`;
          if (btn) btn.disabled = false;
        }
      });
    }
  } catch {}
}

export async function refreshControl() {
  await bootstrap();
  try {
    const ps = await invoke('list_running_models');
    state.running = ps.models || [];
  } catch {
    state.running = [];
  }
  renderControlView();
}

export function initModelsEvents() {
  $('#btn-start-engine').onclick = startEngine;
  $('#btn-refresh-models').onclick = bootstrap;
  $('#btn-refresh-control').onclick = refreshControl;
  $('#custom-pull-form').onsubmit = e => {
    e.preventDefault();
    const name = $('#custom-model-name').value.trim();
    if (name) pullModel(name);
  };
}
