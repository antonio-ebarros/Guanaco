import { $, invoke, state, switchView } from './state.js';
import { renderProfilePill } from './theme.js';
import { t } from './i18n.js';

const STORAGE_KEY = 'guanaco-pro-settings';

export const PERSONAS = [
  {
    id: 'default',
    nameKey: 'personaDefaultName',
    descKey: 'personaDefaultDesc',
    prompt: '',
  },
  {
    id: 'coder',
    nameKey: 'personaCoderName',
    descKey: 'personaCoderDesc',
    prompt:
      'Atue como um Engenheiro de Software Sênior. Forneça código limpo, seguro, modular e direto ao ponto, explicando decisões técnicas com precisão.',
  },
  {
    id: 'writer',
    nameKey: 'personaWriterName',
    descKey: 'personaWriterDesc',
    prompt:
      'Atue como um Escritor e Editor experiente. Priorize clareza estilística, fluidez narrativa, vocabulário rico e adaptação fiel ao tom solicitado.',
  },
  {
    id: 'teacher',
    nameKey: 'personaTeacherName',
    descKey: 'personaTeacherDesc',
    prompt:
      'Atue como um Professor Didático e paciente. Explique conceitos do simples ao avançado usando analogias práticas e exemplos claros.',
  },
  {
    id: 'analyst',
    nameKey: 'personaAnalystName',
    descKey: 'personaAnalystDesc',
    prompt:
      'Atue como um Analista Crítico e Estrategista. Estruture respostas em tópicos objetivos, apontando prós, contras, riscos e próximos passos práticos.',
  },
  {
    id: 'custom',
    nameKey: 'personaCustomName',
    descKey: 'personaCustomDesc',
    prompt: '',
  },
];

export const DEFAULT_PRO_SETTINGS = {
  persona: 'default',
  customPrompt: '',
  temperature: 0.35,
  topP: 0.9,
  numCtx: 4096,
  repeatPenalty: 1.1,
  numGpu: -1,
};

export function getProSettings() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_PRO_SETTINGS };
    const parsed = JSON.parse(raw);
    return {
      persona: PERSONAS.some(p => p.id === parsed.persona) ? parsed.persona : 'default',
      customPrompt: typeof parsed.customPrompt === 'string' ? parsed.customPrompt : '',
      temperature: Number.isFinite(Number(parsed.temperature)) ? Number(parsed.temperature) : 0.35,
      topP: Number.isFinite(Number(parsed.topP)) ? Number(parsed.topP) : 0.9,
      numCtx: Number.isFinite(Number(parsed.numCtx)) ? Number(parsed.numCtx) : 4096,
      repeatPenalty: Number.isFinite(Number(parsed.repeatPenalty)) ? Number(parsed.repeatPenalty) : 1.1,
      numGpu: Number.isFinite(Number(parsed.numGpu)) ? Number(parsed.numGpu) : -1,
    };
  } catch {
    return { ...DEFAULT_PRO_SETTINGS };
  }
}

export function saveProSettings(partial) {
  const next = { ...getProSettings(), ...partial };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  renderChatPersonaSelect();
  return next;
}

export function getEffectiveSystemPrompt() {
  if (!state.license?.active) return null;
  const cfg = getProSettings();
  const personaObj = PERSONAS.find(p => p.id === cfg.persona) || PERSONAS[0];
  const parts = [];
  if (personaObj.prompt) parts.push(personaObj.prompt);
  if (cfg.customPrompt && cfg.customPrompt.trim()) parts.push(cfg.customPrompt.trim());
  return parts.length ? parts.join('\n\n') : null;
}

export async function loadLicenseStatus() {
  try {
    state.license = await invoke('get_license_status');
  } catch {
    state.license = { active: false, tier: 'free', holder: null, email: null, issuedAt: null };
  }
  renderProBadge();
  renderProfilePill();
  renderChatPersonaSelect();
}

export function renderProBadge() {
  const badge = $('#brand-pro-badge');
  if (badge) badge.hidden = !state.license?.active;
}

export function renderChatPersonaSelect() {
  const sel = $('#persona-select');
  if (!sel) return;
  const isPro = Boolean(state.license?.active);
  sel.hidden = !isPro;
  if (!isPro) return;

  const cfg = getProSettings();
  sel.innerHTML = '';
  for (const p of PERSONAS) {
    const opt = document.createElement('option');
    opt.value = p.id;
    opt.textContent = t(p.nameKey);
    sel.append(opt);
  }
  sel.value = cfg.persona;
}

export function initProChatHeader(onRefreshControl) {
  const sel = $('#persona-select');
  if (!sel) return;
  sel.addEventListener('change', () => {
    saveProSettings({ persona: sel.value });
    if (typeof onRefreshControl === 'function') onRefreshControl();
  });
}

export function renderProControlSection(onLicenseChanged) {
  const licCard = $('#pro-license-card');
  const studioCard = $('#pro-studio-card');
  if (!licCard || !studioCard) return;

  const isPro = Boolean(state.license?.active);
  const lic = state.license || {};

  if (isPro) {
    licCard.className = 'info-card pro-card-active';
    licCard.innerHTML = `
      <div class="row-between">
        <div>
          <span class="badge-pro-gold">✨ GUANACO PRO ATIVO</span>
          <h3 class="pro-card-title">${t('proActiveTitle')}</h3>
        </div>
        <button id="btn-pro-deactivate" class="btn-ghost small" type="button">${t('proDeactivateBtn')}</button>
      </div>
      <div class="pro-meta-grid">
        <div><span class="muted">${t('proHolderLabel')}:</span> <strong>${lic.holder || 'Guanaco Pro'}</strong></div>
        ${lic.email ? `<div><span class="muted">E-mail:</span> <strong>${lic.email}</strong></div>` : ''}
        ${lic.issuedAt ? `<div><span class="muted">${t('proIssuedLabel')}:</span> <strong>${lic.issuedAt}</strong></div>` : ''}
        <div><span class="muted">${t('proTypeLabel')}:</span> <strong>${t('proLifetimeVal')}</strong></div>
      </div>
    `;
    licCard.querySelector('#btn-pro-deactivate')?.addEventListener('click', async () => {
      state.license = await invoke('deactivate_license');
      renderProBadge();
      renderProfilePill();
      renderChatPersonaSelect();
      if (typeof onLicenseChanged === 'function') onLicenseChanged();
    });
  } else {
    licCard.className = 'info-card pro-card-upsell';
    licCard.innerHTML = `
      <div class="row-between">
        <div>
          <span class="badge-pro-gold">✨ GUANACO PRO</span>
          <h3 class="pro-card-title">${t('proUpsellTitle')}</h3>
        </div>
        <span class="badge-pro">${t('proOneTimeBadge')}</span>
      </div>
      <p class="muted">${t('proUpsellDesc')}</p>
      <form id="pro-activate-form" class="inline-form">
        <input id="pro-key-input" type="text" placeholder="GUANACO-PRO-..." autocomplete="off" spellcheck="false">
        <button class="btn-primary" type="submit">${t('proActivateBtn')}</button>
      </form>
      <div id="pro-activate-msg" class="pro-msg" hidden></div>
    `;
    const form = licCard.querySelector('#pro-activate-form');
    const input = licCard.querySelector('#pro-key-input');
    const msg = licCard.querySelector('#pro-activate-msg');
    form?.addEventListener('submit', async ev => {
      ev.preventDefault();
      const key = (input?.value || '').trim();
      if (!key) return;
      try {
        state.license = await invoke('activate_license', { key });
        renderProBadge();
        renderProfilePill();
        renderChatPersonaSelect();
        if (typeof onLicenseChanged === 'function') onLicenseChanged();
      } catch (err) {
        if (msg) {
          msg.hidden = false;
          msg.textContent = `${t('errPrefix')}: ${err}`;
        }
      }
    });
  }

  const cfg = getProSettings();
  const activePersona = PERSONAS.find(p => p.id === cfg.persona) || PERSONAS[0];
  studioCard.className = 'info-card pro-studio' + (isPro ? '' : ' locked');

  const personaButtonsHtml = PERSONAS.map(p => {
    const cls = 'persona-chip' + (p.id === cfg.persona ? ' active' : '');
    return `<button type="button" class="${cls}" data-persona="${p.id}" ${isPro ? '' : 'disabled'}>${t(p.nameKey)}</button>`;
  }).join('');

  studioCard.innerHTML = `
    <div class="row-between">
      <div>
        <div class="card-kicker">${t('proStudioKicker')}</div>
        <strong>${t('proPersonasTitle')}</strong>
      </div>
      <span class="${isPro ? 'badge-ok' : 'badge-pro'}">${isPro ? t('profActive') : 'Guanaco Pro'}</span>
    </div>

    <div class="persona-grid">${personaButtonsHtml}</div>
    <div id="persona-desc" class="muted">${t(activePersona.descKey)}</div>

    <div class="pro-field">
      <div class="row-between">
        <label for="pro-system-prompt"><strong>${t('proSysPromptLabel')}</strong></label>
        <span class="muted">${t('proSysPromptHint')}</span>
      </div>
      <textarea id="pro-system-prompt" rows="3" placeholder="${t('proSysPromptPlaceholder')}" ${isPro ? '' : 'disabled'}></textarea>
    </div>

    <div class="pro-params-head row-between">
      <strong>${t('proParamsTitle')}</strong>
      <button id="btn-pro-reset" class="btn-ghost small" type="button" ${isPro ? '' : 'disabled'}>${t('proResetBtn')}</button>
    </div>

    <div class="pro-params-grid">
      <div class="pro-param-item">
        <div class="row-between">
          <label for="pro-temp">${t('proTempLabel')}</label>
          <strong id="pro-temp-val">${cfg.temperature.toFixed(2)}</strong>
        </div>
        <input id="pro-temp" type="range" min="0" max="1.6" step="0.05" value="${cfg.temperature}" ${isPro ? '' : 'disabled'}>
        <span class="muted">${t('proTempDesc')}</span>
      </div>

      <div class="pro-param-item">
        <div class="row-between">
          <label for="pro-top-p">${t('proTopPLabel')}</label>
          <strong id="pro-top-p-val">${cfg.topP.toFixed(2)}</strong>
        </div>
        <input id="pro-top-p" type="range" min="0.1" max="1.0" step="0.05" value="${cfg.topP}" ${isPro ? '' : 'disabled'}>
        <span class="muted">${t('proTopPDesc')}</span>
      </div>

      <div class="pro-param-item">
        <div class="row-between">
          <label for="pro-rep">${t('proRepLabel')}</label>
          <strong id="pro-rep-val">${cfg.repeatPenalty.toFixed(2)}</strong>
        </div>
        <input id="pro-rep" type="range" min="1.0" max="1.5" step="0.05" value="${cfg.repeatPenalty}" ${isPro ? '' : 'disabled'}>
        <span class="muted">${t('proRepDesc')}</span>
      </div>

      <div class="pro-param-item">
        <div class="row-between">
          <label for="pro-ctx">${t('proCtxLabel')}</label>
        </div>
        <select id="pro-ctx" ${isPro ? '' : 'disabled'}>
          <option value="2048" ${cfg.numCtx === 2048 ? 'selected' : ''}>2.048 tokens (Leve / Rápido)</option>
          <option value="4096" ${cfg.numCtx === 4096 ? 'selected' : ''}>4.096 tokens (Padrão)</option>
          <option value="8192" ${cfg.numCtx === 8192 ? 'selected' : ''}>8.192 tokens (Textos longos)</option>
          <option value="16384" ${cfg.numCtx === 16384 ? 'selected' : ''}>16.384 tokens (Documentos grandes)</option>
          <option value="32768" ${cfg.numCtx === 32768 ? 'selected' : ''}>32.768 tokens (Máximo / Alta RAM)</option>
        </select>
        <span class="muted">${t('proCtxDesc')}</span>
      </div>

      <div class="pro-param-item">
        <div class="row-between">
          <label for="pro-gpu">${t('proGpuLabel')}</label>
        </div>
        <select id="pro-gpu" ${isPro ? '' : 'disabled'}>
          <option value="-1" ${cfg.numGpu === -1 ? 'selected' : ''}>${t('proGpuAuto')}</option>
          <option value="16" ${cfg.numGpu === 16 ? 'selected' : ''}>${t('proGpu16')}</option>
          <option value="28" ${cfg.numGpu === 28 ? 'selected' : ''}>${t('proGpu28')}</option>
          <option value="99" ${cfg.numGpu === 99 ? 'selected' : ''}>${t('proGpuMax')}</option>
          <option value="0" ${cfg.numGpu === 0 ? 'selected' : ''}>${t('proGpuCpuOnly')}</option>
        </select>
        <span class="muted">${t('proGpuDesc')}</span>
      </div>
    </div>
  `;

  const promptEl = studioCard.querySelector('#pro-system-prompt');
  if (promptEl) {
    promptEl.value = cfg.customPrompt || '';
  }

  if (!isPro) return;

  studioCard.querySelectorAll('.persona-chip').forEach(btn => {
    btn.addEventListener('click', () => {
      saveProSettings({ persona: btn.dataset.persona });
      renderProControlSection(onLicenseChanged);
    });
  });

  promptEl?.addEventListener('input', () => {
    saveProSettings({ customPrompt: promptEl.value });
  });

  const tempEl = studioCard.querySelector('#pro-temp');
  const tempVal = studioCard.querySelector('#pro-temp-val');
  tempEl?.addEventListener('input', () => {
    const v = Number(tempEl.value);
    if (tempVal) tempVal.textContent = v.toFixed(2);
    saveProSettings({ temperature: v });
  });

  const topPEl = studioCard.querySelector('#pro-top-p');
  const topPVal = studioCard.querySelector('#pro-top-p-val');
  topPEl?.addEventListener('input', () => {
    const v = Number(topPEl.value);
    if (topPVal) topPVal.textContent = v.toFixed(2);
    saveProSettings({ topP: v });
  });

  const repEl = studioCard.querySelector('#pro-rep');
  const repVal = studioCard.querySelector('#pro-rep-val');
  repEl?.addEventListener('input', () => {
    const v = Number(repEl.value);
    if (repVal) repVal.textContent = v.toFixed(2);
    saveProSettings({ repeatPenalty: v });
  });

  const ctxEl = studioCard.querySelector('#pro-ctx');
  ctxEl?.addEventListener('change', () => {
    saveProSettings({ numCtx: Number(ctxEl.value) });
  });

  const gpuEl = studioCard.querySelector('#pro-gpu');
  gpuEl?.addEventListener('change', () => {
    saveProSettings({ numGpu: Number(gpuEl.value) });
  });

  studioCard.querySelector('#btn-pro-reset')?.addEventListener('click', () => {
    saveProSettings({ ...DEFAULT_PRO_SETTINGS });
    renderProControlSection(onLicenseChanged);
  });
}

let ggufMode = 'file'; // 'file' | 'base'

export function renderProModelsBuilder(onModelCreated) {
  const card = $('#pro-gguf-card');
  if (!card) return;
  const isPro = Boolean(state.license?.active);
  card.className = 'info-card pro-studio' + (isPro ? '' : ' locked');

  const baseOptions = (state.installed || [])
    .map(m => `<option value="${m.name}">${m.name}</option>`)
    .join('');

  card.innerHTML = `
    <div class="row-between">
      <div>
        <div class="card-kicker">${t('proGgufKicker')}</div>
        <strong>${t('proGgufTitle')}</strong>
      </div>
      <span class="${isPro ? 'badge-ok' : 'badge-pro-gold'}">✨ Guanaco Pro</span>
    </div>
    <p class="muted">${t('proGgufDesc')}</p>

    <div class="persona-grid">
      <button type="button" class="persona-chip ${ggufMode === 'file' ? 'active' : ''}" data-gmode="file" ${isPro ? '' : 'disabled'}>${t('proGgufModeFile')}</button>
      <button type="button" class="persona-chip ${ggufMode === 'base' ? 'active' : ''}" data-gmode="base" ${isPro ? '' : 'disabled'}>${t('proGgufModeBase')}</button>
    </div>

    <div id="gguf-source-wrap">
      ${
        ggufMode === 'file'
          ? `<div class="inline-form">
               <input id="gguf-path-input" type="text" placeholder="${t('proGgufPathPlaceholder')}" ${isPro ? '' : 'disabled'}>
               <button id="btn-pick-gguf" class="btn-ghost" type="button" ${isPro ? '' : 'disabled'}>${t('proGgufPickBtn')}</button>
             </div>`
          : `<div class="pro-field">
               <select id="gguf-base-select" class="gguf-select" ${isPro ? '' : 'disabled'}>
                 ${baseOptions || `<option value="">${t('noInstalledModels')}</option>`}
               </select>
             </div>`
      }
    </div>

    <div class="pro-params-grid">
      <div class="pro-field">
        <label for="gguf-name-input"><strong>${t('proGgufNameLabel')}</strong></label>
        <input id="gguf-name-input" class="gguf-text-input" type="text" placeholder="${t('proGgufNamePlaceholder')}" ${isPro ? '' : 'disabled'}>
      </div>
      <div class="pro-field">
        <label for="gguf-temp-input"><strong>${t('proTempLabel')}</strong></label>
        <input id="gguf-temp-input" class="gguf-text-input" type="number" min="0" max="2" step="0.05" value="0.35" ${isPro ? '' : 'disabled'}>
      </div>
    </div>

    <div class="pro-field">
      <label for="gguf-sys-input"><strong>${t('proGgufSysLabel')}</strong></label>
      <textarea id="gguf-sys-input" rows="2" placeholder="${t('proGgufSysPlaceholder')}" ${isPro ? '' : 'disabled'}></textarea>
    </div>

    <div id="gguf-status-msg" class="muted" hidden></div>

    <div class="row-between">
      <span></span>
      ${
        isPro
          ? `<button id="btn-create-gguf" class="btn-primary" type="button">${t('proGgufCreateBtn')}</button>`
          : `<button id="btn-unlock-gguf" class="btn-ghost small" type="button">${t('profLockedBtn')}</button>`
      }
    </div>
  `;

  if (!isPro) {
    card.querySelector('#btn-unlock-gguf')?.addEventListener('click', () => {
      switchView('control');
      setTimeout(() => {
        $('#pro-license-card')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        $('#pro-key-input')?.focus();
      }, 150);
    });
    return;
  }

  card.querySelectorAll('[data-gmode]').forEach(btn => {
    btn.addEventListener('click', () => {
      ggufMode = btn.dataset.gmode;
      renderProModelsBuilder(onModelCreated);
    });
  });

  const pathInput = card.querySelector('#gguf-path-input');
  const nameInput = card.querySelector('#gguf-name-input');
  const statusEl = card.querySelector('#gguf-status-msg');

  card.querySelector('#btn-pick-gguf')?.addEventListener('click', async () => {
    try {
      const picked = await invoke('pick_gguf_file');
      if (picked && pathInput) {
        pathInput.value = picked;
        if (nameInput && !nameInput.value.trim()) {
          const base = picked.split(/[\\/]/).pop() || '';
          const slug = base
            .replace(/\.gguf$/i, '')
            .toLowerCase()
            .replace(/[^a-z0-9._-]+/g, '-')
            .replace(/^-+|-+$/g, '')
            .slice(0, 36);
          if (slug) nameInput.value = slug;
        }
      }
    } catch (err) {
      if (statusEl) {
        statusEl.hidden = false;
        statusEl.textContent = `${t('errPrefix')}: ${err}`;
      }
    }
  });

  card.querySelector('#btn-create-gguf')?.addEventListener('click', async () => {
    const btn = card.querySelector('#btn-create-gguf');
    const name = (nameInput?.value || '').trim();
    const fromSource =
      ggufMode === 'file'
        ? (pathInput?.value || '').trim()
        : (card.querySelector('#gguf-base-select')?.value || '').trim();
    const sysPrompt = (card.querySelector('#gguf-sys-input')?.value || '').trim();
    const tempVal = Number(card.querySelector('#gguf-temp-input')?.value);

    if (!fromSource || !name) {
      if (statusEl) {
        statusEl.hidden = false;
        statusEl.textContent = t('proGgufMissingFields');
      }
      return;
    }

    if (btn) btn.disabled = true;
    if (statusEl) {
      statusEl.hidden = false;
      statusEl.textContent = t('proGgufCreatingStatus');
    }
    try {
      const created = await invoke('create_custom_model', {
        req: {
          name,
          fromSource,
          isGgufFile: ggufMode === 'file',
          systemPrompt: sysPrompt || null,
          temperature: Number.isFinite(tempVal) ? tempVal : 0.35,
          numCtx: null,
        },
      });
      if (statusEl) {
        statusEl.hidden = false;
        statusEl.textContent = t('proGgufSuccessMsg', { name: created });
      }
      if (typeof onModelCreated === 'function') await onModelCreated(created);
    } catch (err) {
      if (statusEl) {
        statusEl.hidden = false;
        statusEl.textContent = `${t('errPrefix')}: ${err}`;
      }
    } finally {
      if (btn) btn.disabled = false;
    }
  });
}
