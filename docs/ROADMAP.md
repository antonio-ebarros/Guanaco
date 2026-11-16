# Plano de Evolução do Guanaco

Este documento organiza o crescimento do projeto em fases claras, mantendo o núcleo simples para usuários leigos e a base preparada para evoluir sem bagunça.

---

## Fase 1 — Estabilidade e Confiança (Atual)
Objetivo: garantir que o básico funcione perfeitamente antes de adicionar novos recursos.

- [x] Interface limpa com temas claro/escuro, vidro fosco e fonte ajustável (`A-` / `A+`).
- [x] Detecção de hardware realista (RAM livre agora, VRAM livre, reserva do SO, CPU e modelos já carregados).
- [x] Escrita atômica + backup automático (`conversations.bak.json`) para não perder histórico local.
- [ ] Validação manual completa em Linux e Windows:
  - Telas (`Conversa`, `Modelos`, `Controle`) nos dois temas.
  - Download de modelo com barra de progresso real.
  - Execução (chat com streaming + markdown + anexos).
  - Remoção de modelo e liberação de RAM/VRAM.
  - Persistência das conversas após fechar e reabrir o aplicativo.

---

## Fase 2 — Modularização para Crescer Sem Bagunça
Objetivo: dividir responsabilidades antes que `lib.rs` e `app.js` fiquem grandes demais.

- **Backend (`src-tauri/src/`):**
  - `hardware.rs`: detecção de RAM, CPU, GPU/VRAM e reserva do SO.
  - `engine.rs` (trait `AiEngine`): interface comum de motor local (começando com `ollama.rs`), permitindo suportar novos motores no futuro sem reescrever a UI.
  - `storage.rs`: persistência atômica, backup e futuras configurações do usuário.
- **Frontend (`src/`):**
  - Separar em módulos ES (`state.js`, `chat.js`, `models.js`, `control.js`, `theme.js`).

---

## Fase 3 — Ecossistema Guanaco (Open Core)
Base aberta sob licença **Apache-2.0** (`Guanaco`), com espaço limpo para extensões comerciais em repositórios separados:

1. **🦙 Guanaco (Gratuito e Aberto — este repositório)**
   - Chat local, gerenciamento de modelos, detecção de hardware, controle de memória e privacidade 100% offline.
2. **🦙 Guanaco Pro (Comercial)**
   - Perfis avançados de IA, múltiplos mecanismos locais, ferramentas de produtividade e personalização avançada.
3. **🦙 Guanaco Agent (Comercial / Módulo independente)**
   - Execução assistida de tarefas com aprovação explícita do usuário (arquivos autorizados, comandos aprovados, automações e agentes especializados).
