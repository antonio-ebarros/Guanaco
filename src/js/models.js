import { $, PRESETS, fmtGb, invoke, state, switchView } from './state.js';

let onRenderChat = () => {};

export function setChatRenderer(fn) {
  onRenderChat = fn;
}

export async function bootstrap() {
  state.sysInfo = await invoke('get_system_info');
  const dot = $('#engine-dot');
  const lbl = $('#engine-label');
  const mem = $('#engine-mem');
  const btnStart = $('#btn-start-engine');

  mem.textContent = `${state.sysInfo.totalRamGb} GB RAM • ${state.sysInfo.cpuCores} núcleos`;
  if (state.sysInfo.ollamaRunning) {
    dot.className = 'dot ok';
    lbl.textContent = 'Motor local ativo';
    btnStart.hidden = true;
    await refreshModels();
  } else if (state.sysInfo.ollamaInstalled) {
    dot.className = 'dot err';
    lbl.textContent = 'Motor desligado';
    btnStart.hidden = false;
    state.installed = [];
  } else {
    dot.className = 'dot err';
    lbl.textContent = 'Motor não preparado';
    btnStart.hidden = true;
    state.installed = [];
  }
  renderModelsView();
  if (!state.busy) onRenderChat();
}

export async function startEngine() {
  $('#engine-label').textContent = 'Ligando motor…';
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
    if (st) st.textContent = `Erro: ${e}`;
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
  if (!s) return { cls: 'badge-ok', label: 'Verificando PC…' };
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
    return { cls: 'badge-ok', label: 'Roda rápido na GPU agora' };
  }
  if (budgetNow >= need) {
    if (!hasGpu && (s.cpuCores || 0) <= 2 && need > 2.5) {
      return { cls: 'badge-warn', label: 'Cabe na RAM, mas CPU lenta' };
    }
    return { cls: 'badge-ok', label: hasGpu ? 'Roda bem agora (GPU + RAM)' : 'Roda bem na RAM agora' };
  }
  if (state.running.length && budgetAfterUnload >= need) {
    return { cls: 'badge-warn', label: 'Libere modelos ativos para rodar' };
  }
  if (maxHardwareBudget >= need) {
    return { cls: 'badge-warn', label: `Pouca memória livre agora (~${need} GB)` };
  }
  return { cls: 'badge-err', label: `Pesado demais (~${need} GB em uso)` };
}

export function renderModelsView() {
  const s = state.sysInfo;
  if (s) {
    const osReserve = s.osReservedGb ?? 1.5;
    const runningMemGb = state.running.reduce((acc, m) => acc + (m.size || 0) / 1073741824, 0);
    const gpuText = s.gpuName
      ? `GPU: <b>${s.gpuName}</b> (${s.gpuVramFreeGb} GB livres de ${s.gpuVramTotalGb} GB VRAM)`
      : `Sem GPU NVIDIA dedicada (rodará na CPU)`;
    const runningHtml = state.running.length
      ? `<div class="row-between" style="margin-top:4px;">
           <span class="muted">Modelos carregados agora: <b>${state.running.length}</b> ocupando <b>${runningMemGb.toFixed(2)} GB</b></span>
           <button id="btn-unload-all-models" class="btn-ghost small" type="button">Liberar memória</button>
         </div>`
      : `<div class="muted">Nenhum modelo carregado na memória no momento.</div>`;

    $('#hw-banner').innerHTML = `
      <div><strong>Memória livre agora:</strong> <b>${s.freeRamGb} GB</b> livres de ${s.totalRamGb} GB totais (reservando ~${osReserve} GB para o sistema)</div>
      <div class="muted">CPU: ${s.cpuName} (${s.cpuCores} núcleos) • ${gpuText}</div>
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
  if (!state.installed.length) {
    list.innerHTML = `<div class="list-row"><span class="muted">Nenhum modelo baixado ainda.</span></div>`;
  } else {
    for (const m of state.installed) {
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
        if (!state.busy) onRenderChat();
      };
      list.append(row);
    }
  }
}

export async function pullModel(name) {
  if (!state.sysInfo?.ollamaRunning) {
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
    if (!state.busy) onRenderChat();
  } catch (e) {
    $('#pull-status').textContent = `Erro: ${e}`;
  }
}

export async function refreshControl() {
  await bootstrap();
  const s = state.sysInfo;
  $('#ctrl-hw').textContent = `${s.freeRamGb} GB livres de ${s.totalRamGb} GB RAM`;
  $('#ctrl-gpu').textContent = s.gpuName
    ? `CPU: ${s.cpuName} (${s.cpuCores} núcleos) • GPU: ${s.gpuName} (${s.gpuVramFreeGb}/${s.gpuVramTotalGb} GB VRAM livres)`
    : `CPU: ${s.cpuName} (${s.cpuCores} núcleos) • Sem GPU dedicada`;

  const runBox = $('#running-list');
  runBox.innerHTML = '';
  try {
    const ps = await invoke('list_running_models');
    state.running = ps.models || [];
  } catch {
    state.running = [];
  }
  if (!state.running.length) {
    runBox.innerHTML = `<div class="list-row"><span class="muted">Nenhum modelo carregado na memória no momento.</span></div>`;
  } else {
    for (const r of state.running) {
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

  const totalDisk = state.installed.reduce((acc, m) => acc + (m.size || 0), 0);
  $('#storage-card').innerHTML = `
    <div><strong>Pasta dos modelos:</strong> <code>${s.modelsDir}</code></div>
    <div class="muted">${state.installed.length} modelo(s) salvo(s) ocupando <b>${fmtGb(totalDisk)}</b> no disco.</div>
  `;
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
