import { invoke } from './state.js';
import { getLang, getSpeechLang, t } from './i18n.js';

let activeSpeakBtn = null;
let activeAudio = null;

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
  if (activeAudio) {
    activeAudio.pause();
    activeAudio.src = '';
    activeAudio = null;
  }
  if (activeSpeakBtn) {
    activeSpeakBtn.textContent = t('speak');
    activeSpeakBtn.classList.remove('active');
    activeSpeakBtn = null;
  }
}

export async function toggleSpeak(btn, text) {
  const wasSame = activeSpeakBtn === btn;
  stopSpeaking();
  if (wasSame) return;

  const clean = cleanTextForSpeech(text);
  if (!clean) return;

  activeSpeakBtn = btn;
  btn.classList.add('active');

  try {
    const st = await invoke('get_tts_status');
    if (st?.installed && st?.espeakReady) {
      btn.textContent = '⏳ …';
      const wavDataUrl = await invoke('synthesize_kokoro', {
        text: clean,
        lang: getLang(),
      });
      if (activeSpeakBtn !== btn) return; // foi parado enquanto sintetizava
      const audio = new Audio(wavDataUrl);
      activeAudio = audio;
      btn.textContent = t('speakStop');
      audio.onended = () => {
        if (activeSpeakBtn === btn) stopSpeaking();
      };
      audio.onerror = () => {
        if (activeSpeakBtn === btn) stopSpeaking();
      };
      await audio.play();
      return;
    }
  } catch {
    // Fallback automático para speechSynthesis do sistema
  }

  if (activeSpeakBtn !== btn) return;
  if (!('speechSynthesis' in window)) {
    btn.textContent = t('speakUnavailable');
    setTimeout(() => stopSpeaking(), 1800);
    return;
  }

  const utter = new SpeechSynthesisUtterance(clean);
  utter.lang = getSpeechLang();
  utter.rate = 1.0;
  btn.textContent = t('speakStop');
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
