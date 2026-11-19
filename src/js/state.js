export const { invoke } = window.__TAURI__.core;
export const { listen } = window.__TAURI__.event;

export const $ = s => document.querySelector(s);
export const $$ = s => [...document.querySelectorAll(s)];

export const PRESETS = [
  { id: 'qwen2.5:1.5b', nameKey: 'preset_fast_name', descKey: 'preset_fast_desc', sizeGb: 1.0, neededMemGb: 1.8 },
  { id: 'llama3.2:3b', nameKey: 'preset_bal_name', descKey: 'preset_bal_desc', sizeGb: 2.0, neededMemGb: 2.8 },
  { id: 'moondream', nameKey: 'preset_vis_lite_name', descKey: 'preset_vis_lite_desc', sizeGb: 1.7, neededMemGb: 2.3 },
  { id: 'gemma3:4b', nameKey: 'preset_vis_pro_name', descKey: 'preset_vis_pro_desc', sizeGb: 3.3, neededMemGb: 4.2 },
  { id: 'qwen2.5:7b', nameKey: 'preset_cap_name', descKey: 'preset_cap_desc', sizeGb: 4.7, neededMemGb: 5.8 },
];

export const state = {
  sysInfo: null,
  license: { active: false, tier: 'free', holder: null, email: null, issuedAt: null },
  installed: [],
  running: [],
  conversations: [],
  activeConvId: null,
  attachments: [],
  busy: false,
  streamingConvId: null,
  thinkingLabel: null,
  thinkingStartedAt: 0,
  activeStreamBubble: null,
  streamCancelled: false,
};

export function fmtGb(bytes) {
  return (bytes / 1073741824).toFixed(2) + ' GB';
}

export function switchView(name, onControl) {
  $$('.nav-tab').forEach(b => b.classList.toggle('active', b.dataset.view === name));
  $$('.view').forEach(v => v.classList.toggle('active', v.id === `view-${name}`));
  if (name === 'control' && typeof onControl === 'function') onControl();
}
