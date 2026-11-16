import { getSpeechLang, t } from './i18n.js';

let activeSpeakBtn = null;

function cleanTextForSpeech(mdText) {
  return String(mdText || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/[#>*_~-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function stopSpeaking() {
  if ('speechSynthesis' in window) {
    window.speechSynthesis.cancel();
  }
  if (activeSpeakBtn) {
    activeSpeakBtn.textContent = t('speak');
    activeSpeakBtn.classList.remove('active');
    activeSpeakBtn = null;
  }
}

export function toggleSpeak(btn, text) {
  if (!('speechSynthesis' in window)) {
    btn.textContent = t('speakUnavailable');
    setTimeout(() => { btn.textContent = t('speak'); }, 1800);
    return;
  }

  const wasSame = activeSpeakBtn === btn;
  stopSpeaking();
  if (wasSame) return;

  const clean = cleanTextForSpeech(text);
  if (!clean) return;

  const utter = new SpeechSynthesisUtterance(clean);
  utter.lang = getSpeechLang();
  utter.rate = 1.0;

  activeSpeakBtn = btn;
  btn.textContent = t('speakStop');
  btn.classList.add('active');

  utter.onend = () => {
    if (activeSpeakBtn === btn) stopSpeaking();
  };
  utter.onerror = () => {
    if (activeSpeakBtn === btn) stopSpeaking();
  };

  window.speechSynthesis.speak(utter);
}

export async function copyResponseText(btn, text) {
  const val = String(text || '').trim();
  if (!val) return;
  try {
    await navigator.clipboard.writeText(val);
    btn.textContent = t('copied');
    btn.classList.add('active');
    setTimeout(() => {
      btn.textContent = t('copy');
      btn.classList.remove('active');
    }, 1600);
  } catch {
    btn.textContent = t('copyError');
    setTimeout(() => { btn.textContent = t('copy'); }, 1500);
  }
}
