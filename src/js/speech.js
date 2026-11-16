import { invoke } from './state.js';
import { getLang, getSpeechLang, t } from './i18n.js';

let activeSpeakBtn = null;
let activeAudio = null;
let speakSession = 0;

function cleanTextForSpeech(mdText) {
  return String(mdText || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/[#>*_~-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function splitSentences(text, maxLen = 180) {
  const parts = text.split(/(?<=[.!?。！？])\s+/).filter(Boolean);
  const chunks = [];
  for (const part of parts) {
    if (part.length <= maxLen) {
      chunks.push(part);
    } else {
      for (let i = 0; i < part.length; i += maxLen) {
        chunks.push(part.slice(i, i + maxLen));
      }
    }
  }
  return chunks.length ? chunks : [text];
}

export function stopSpeaking() {
  speakSession++;
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

function playWav(dataUrl, sessionId) {
  return new Promise((resolve, reject) => {
    if (sessionId !== speakSession) {
      resolve();
      return;
    }
    const audio = new Audio(dataUrl);
    activeAudio = audio;
    audio.onended = () => {
      if (activeAudio === audio) activeAudio = null;
      resolve();
    };
    audio.onerror = () => {
      if (activeAudio === audio) activeAudio = null;
      reject(new Error('audio playback error'));
    };
    audio.play().catch(reject);
  });
}

export async function toggleSpeak(btn, text) {
  const wasSame = activeSpeakBtn === btn;
  stopSpeaking();
  if (wasSame) return;

  const clean = cleanTextForSpeech(text);
  if (!clean) return;

  const sessionId = ++speakSession;
  activeSpeakBtn = btn;
  btn.classList.add('active');

  try {
    const lang = getLang();
    const st = await invoke('get_tts_status', { lang });
    if (sessionId !== speakSession) return;
    if (st?.installed) {
      btn.textContent = '⏳ …';
      const chunks = splitSentences(clean);
      for (let i = 0; i < chunks.length; i++) {
        if (sessionId !== speakSession) return;
        const wavDataUrl = await invoke('synthesize_piper', {
          text: chunks[i],
          lang,
        });
        if (sessionId !== speakSession) return;
        btn.textContent = t('speakStop');
        await playWav(wavDataUrl, sessionId);
      }
      if (sessionId === speakSession && activeSpeakBtn === btn) stopSpeaking();
      return;
    }
  } catch {
    // Fallback automático para speechSynthesis do sistema
  }

  if (sessionId !== speakSession || activeSpeakBtn !== btn) return;
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
    if (sessionId === speakSession && activeSpeakBtn === btn) stopSpeaking();
  };
  utter.onerror = () => {
    if (sessionId === speakSession && activeSpeakBtn === btn) stopSpeaking();
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
