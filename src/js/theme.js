import { $ } from './state.js';
import { applyI18n, t } from './i18n.js';

export function applyTheme(theme) {
  const th = theme === 'dark' ? 'dark' : 'light';
  document.documentElement.dataset.theme = th;
  localStorage.setItem('guanaco-theme', th);
  const btn = $('#btn-theme');
  if (btn) btn.textContent = th === 'dark' ? t('themeLight') : t('themeDark');
}

export function applyFontSize(px) {
  const size = Math.min(26, Math.max(14, Number(px) || 18));
  document.documentElement.style.setProperty('--chat-font-size', `${size}px`);
  localStorage.setItem('guanaco-font-size', String(size));
  const lbl = $('#font-size-label');
  if (lbl) lbl.textContent = `${size}px`;
}

export function getResponseProfile() {
  const saved = localStorage.getItem('guanaco-profile');
  return saved === 'balanced' ? 'balanced' : 'conservative';
}

export function setResponseProfile(profile) {
  const p = profile === 'balanced' ? 'balanced' : 'conservative';
  localStorage.setItem('guanaco-profile', p);
  renderProfilePill();
}

export function renderProfilePill() {
  const pill = $('#btn-profile-pill');
  if (!pill) return;
  const p = getResponseProfile();
  pill.textContent = p === 'balanced' ? t('profBalName') : t('profConsName');
}

export function initPreferences(onLangChange) {
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

  $('#lang-select')?.addEventListener('change', ev => {
    localStorage.setItem('guanaco-lang', ev.target.value);
    applyI18n();
    applyTheme(localStorage.getItem('guanaco-theme') || 'light');
    renderProfilePill();
    if (typeof onLangChange === 'function') onLangChange();
  });

  applyI18n();
  applyTheme(localStorage.getItem('guanaco-theme') || 'light');
  applyFontSize(localStorage.getItem('guanaco-font-size') || 18);
  renderProfilePill();

  const ecoToggle = $('#eco-mode-toggle');
  if (ecoToggle) {
    ecoToggle.checked = localStorage.getItem('guanaco-eco-mode') === '1';
    ecoToggle.addEventListener('change', () => {
      localStorage.setItem('guanaco-eco-mode', ecoToggle.checked ? '1' : '0');
    });
  }
}
