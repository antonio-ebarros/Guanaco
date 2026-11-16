import { $ } from './state.js';

export function applyTheme(theme) {
  const t = theme === 'dark' ? 'dark' : 'light';
  document.documentElement.dataset.theme = t;
  localStorage.setItem('guanaco-theme', t);
  const btn = $('#btn-theme');
  if (btn) btn.textContent = t === 'dark' ? '☀️ Claro' : '🌙 Escuro';
}

export function applyFontSize(px) {
  const size = Math.min(26, Math.max(14, Number(px) || 18));
  document.documentElement.style.setProperty('--chat-font-size', `${size}px`);
  localStorage.setItem('guanaco-font-size', String(size));
  const lbl = $('#font-size-label');
  if (lbl) lbl.textContent = `${size}px`;
}

export function initPreferences() {
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

  const ecoToggle = $('#eco-mode-toggle');
  if (ecoToggle) {
    ecoToggle.checked = localStorage.getItem('guanaco-eco-mode') === '1';
    ecoToggle.addEventListener('change', () => {
      localStorage.setItem('guanaco-eco-mode', ecoToggle.checked ? '1' : '0');
    });
  }
}
