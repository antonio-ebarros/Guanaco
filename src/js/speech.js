let activeSpeakBtn = null;

function cleanTextForSpeech(mdText) {
  return String(mdText || '')
    .replace(/```[\s\S]*?```/g, ' bloco de código ')
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
    activeSpeakBtn.textContent = '🔊 Ouvir';
    activeSpeakBtn.classList.remove('active');
    activeSpeakBtn = null;
  }
}

export function toggleSpeak(btn, text) {
  if (!('speechSynthesis' in window)) {
    btn.textContent = 'Indisponível';
    setTimeout(() => { btn.textContent = '🔊 Ouvir'; }, 1800);
    return;
  }

  const wasSame = activeSpeakBtn === btn;
  stopSpeaking();
  if (wasSame) return;

  const clean = cleanTextForSpeech(text);
  if (!clean) return;

  const utter = new SpeechSynthesisUtterance(clean);
  utter.lang = 'pt-BR';
  utter.rate = 1.0;

  activeSpeakBtn = btn;
  btn.textContent = '⏹ Parar';
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
    const prev = btn.textContent;
    btn.textContent = '✓ Copiado';
    btn.classList.add('active');
    setTimeout(() => {
      btn.textContent = prev;
      btn.classList.remove('active');
    }, 1600);
  } catch {
    btn.textContent = 'Erro';
    setTimeout(() => { btn.textContent = '📋 Copiar'; }, 1500);
  }
}
