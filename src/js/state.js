export const { invoke } = window.__TAURI__.core;
export const { listen } = window.__TAURI__.event;

export const $ = s => document.querySelector(s);
export const $$ = s => [...document.querySelectorAll(s)];

export const PRESETS = [
  { id: 'qwen2.5:1.5b', name: 'Rápido e Leve', sizeGb: 1.0, neededMemGb: 1.8, desc: 'Ideal para computadores básicos e respostas rápidas no dia a dia.' },
  { id: 'llama3.2:3b', name: 'Equilibrado', sizeGb: 2.0, neededMemGb: 2.8, desc: 'Ótimo equilíbrio entre velocidade, português claro e resumos.' },
  { id: 'moondream', name: 'Visão Leve (Imagens)', sizeGb: 1.7, neededMemGb: 2.3, desc: 'Analisa fotos e prints sem pesar em computadores mais simples.' },
  { id: 'gemma3:4b', name: 'Visão Avançada', sizeGb: 3.3, neededMemGb: 4.2, desc: 'Mais detalhado para imagens e textos, ideal para GPUs com 6 GB+ ou bastante RAM.' },
  { id: 'qwen2.5:7b', name: 'Mais Capaz', sizeGb: 4.7, neededMemGb: 5.8, desc: 'Melhor raciocínio e textos longos para PCs com mais memória.' },
];

export const state = {
  sysInfo: null,
  installed: [],
  running: [],
  conversations: [],
  activeConvId: null,
  attachments: [],
  busy: false,
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
