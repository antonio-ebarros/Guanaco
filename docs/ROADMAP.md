# Plano de Evolução do Guanaco

Este documento organiza o crescimento do projeto em fases claras, mantendo o núcleo simples para usuários leigos e a base preparada para evoluir sem bagunça.

---

## Fase 1 — Estabilidade e Confiança (Atual)
Objetivo: garantir que o básico funcione perfeitamente antes de adicionar novos recursos.

- [x] Interface limpa com temas claro/escuro, vidro fosco e fonte ajustável (`A-` / `A+`).
- [x] Detecção de hardware realista (RAM livre agora, VRAM livre, reserva do SO, CPU e modelos já carregados).
- [x] Escrita atômica + backup automático (`conversations.bak.json`) para não perder histórico local.
- [x] Validação manual completa no Linux (telas, download, remoção, execução com GPU, Modo Econômico, instalação do motor em 1 clique e persistência).
- [ ] Validação final dos instaladores Windows (`.exe`) e Linux (`.deb`/`.AppImage`).

---

## Fase 2 — Modularização para Crescer Sem Bagunça
Objetivo: dividir responsabilidades antes que `lib.rs` e `app.js` fiquem grandes demais.

- **Backend (`src-tauri/src/`):**
  - [x] `hardware.rs`: detecção de RAM, CPU, GPU/VRAM e reserva do SO.
  - [x] `ollama.rs`: instalação, inicialização, modelos, streaming/cancelamento e cleanup.
  - [x] `storage.rs`: persistência atômica e backup de conversas.
  - [x] `engine.rs` (trait `AiEngine`): abstração comum para suportar outros motores locais no futuro (ex.: `llamafile` / `llama.cpp`) sem reescrever a UI.
- **Frontend (`src/`):**
  - [x] Separar `app.js` em módulos ES (`state.js`, `chat.js`, `models.js`, `theme.js`, `speech.js`).
  - [x] Acessibilidade e utilidade: botões discretos **"🔊 Ouvir"** (ler em voz alta) e **"📋 Copiar"** nas respostas.
  - [x] Melhoria de voz: integrado TTS local natural (**Piper TTS** sob demanda em processo separado) e suporte a 3 idiomas (`Português`, `English`, `中文`).

---

## Fase 3 — Ecossistema Guanaco & Guanaco Pro (Concluída em v0.4.0)
Repositório unificado sob licença **Apache-2.0** (`Guanaco`), com ativação vitalícia 100% offline via assinatura criptográfica **Ed25519**:

1. **🦙 Guanaco (Gratuito e Aberto)**
   - [x] Chat local, gerenciamento de modelos, detecção de hardware (NVIDIA, AMD e Intel), controle de memória, Piper TTS neural offline, 3 idiomas (`PT`, `EN`, `中文`) e Perfis **Conservador** e **Equilibrado**.
2. **✨ Guanaco Pro (Pagamento único, vitalício e 100% offline)**
   - [x] Validação de licença offline com chave pública `Ed25519` (`src-tauri/src/license.rs`).
   - [x] Desbloqueio do **Perfil Livre (`🔓 Livre`)**.
   - [x] **Personalidades** (5 modos prontos + criador de personalidades salvas pelo usuário) e **Contexto Global / System Prompt editável**.
   - [x] **Parâmetros Avançados** (`temperature`, `top_p`, `repeat_penalty`, `num_ctx`, `num_gpu` com auto-fallback inteligente de VRAM).
   - [x] **Importador de modelos `.GGUF` locais** na aba Modelos.
   - [x] **Ferramentas e Extensões Locais** (`src-tauri/src/tools.rs`): Pasta de Conhecimento Local (RAG Offline) e Calculadora Exata + Contexto de Tempo/Sistema.
   - [x] **Suporte Multi-Motor (`AiEngine`)**: alternância entre **Ollama (`127.0.0.1:11434`)** e **Mozilla `llamafile` / `llama.cpp` (`127.0.0.1:8081`)**.
3. **🤖 Guanaco Agent (Próxima Fronteira / Futuro)**
   - [ ] Execução assistida de tarefas com aprovação explícita do usuário (automações e agentes especializados).
