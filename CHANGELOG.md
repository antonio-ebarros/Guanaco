# Changelog

## [0.3.0] - 2026-11-16
- Suporte completo a **3 idiomas** na interface (**Português**, **English**, **中文**) via `src/js/i18n.js`.
- Voz natural offline com **Piper TTS** (módulo `src-tauri/src/tts.rs`, download sob demanda na aba Controle, execução em processo separado compatível com CPUs antigas e fallback automático para voz do sistema).
- Prompt de sistema padrão embutido e oculto (`src-tauri/prompts/guanaco_system.txt`) com contexto sobre o aplicativo, suas abas e autoria, e créditos adicionados no `README.md` (incluindo **Edresson Casanova** / `TTS-Portuguese-Corpus`).

## [0.2.0] - 2026-11-16
- Instalação automática do motor local Ollama em 1 clique (Linux e Windows), sem terminal.
- Recomendação inteligente de modelos (RAM livre, VRAM livre, reserva do SO, CPU e modelos ativos) + preset leve `moondream` e **Modo econômico (poupar CPU)**.
- Miniaturas de imagens (redimensionadas até 640px), selos limpos de documentos anexados, indicador animado **"Pensando… Xs"**, botão **Parar** e encerramento automático do runner ao cancelar ou sair.
- Botões discretos **🔊 Ouvir** e **📋 Copiar** nas respostas.
- Arquitetura modularizada (Fase 2): backend Rust (`engine.rs` com trait `AiEngine`, `hardware.rs`, `ollama.rs`, `storage.rs`) e frontend em ES Modules (`src/js/`).
- Licença oficial **Apache-2.0** adicionada ao repositório.

## [0.1.0] - Inicial
- Estrutura inicial do projeto desktop multiplataforma (Windows e Linux) com **Tauri v2**.
- Documentação inicial (`README.md`, `docs/ARQUITETURA.md`) e pipeline de build em GitHub Actions (`.github/workflows/build.yml`).
- Detecção de hardware local (RAM, CPU, GPU) e painel de controle/transparência para modelos locais via Ollama.
- Interface de conversa limpa com editor Markdown ao vivo e catálogo guiado de modelos.
- Nova identidade visual com logo oficial `guanaco>_`, ícone do aplicativo 512x512, tema claro e tema escuro, efeito de vidro fosco na caixa de digitação e respostas com letra grande (`18px` por padrão) com controle rápido `A-`/`A+`.
- Recomendação inteligente de modelos baseada em RAM disponível no momento, VRAM disponível na GPU, núcleos da CPU, reserva do sistema operacional e modelos já carregados na memória.
- Licença oficial **Apache-2.0** adicionada ao repositório (`LICENSE`, `Cargo.toml`, `package.json` e `README.md`).
- Persistência atômica com backup automático (`conversations.bak.json`) em `save_conversations`/`load_conversations` e plano de evolução documentado em `docs/ROADMAP.md`.
- Miniatura de imagens enviadas (qualquer formato) nos chips e nas bolhas do chat, botão **Parar** para cancelar geração em andamento e limpeza automática do motor/geração ao fechar o app.
- Indicador animado **"Pensando… Xs"** / **"Analisando imagem… Xs"** durante a geração, selo limpo (`📄 arquivo`) para documentos anexados sem poluir o chat, correção de balão vazio ao cancelar e descarregamento automático do modelo anterior ao trocar de modelo.
- Adicionado preset de visão leve (`moondream`) para computadores com menos VRAM/RAM e opção **Modo econômico (poupar CPU)** limitando `num_thread` e `num_ctx`.
- Descarregamento automático do modelo antes da remoção (`delete_model`) e instalação automática do motor local em 1 clique (`install_ollama`) com progresso ao vivo para Linux e Windows.
- Fase 2 iniciada: backend Rust modularizado em `hardware.rs`, `ollama.rs`, `storage.rs` e `lib.rs`.
- Frontend modularizado em `src/js/` (`state.js`, `theme.js`, `models.js`, `chat.js`, `speech.js`) e botões discretos **🔊 Ouvir** e **📋 Copiar** nas respostas.
- Fase 2 concluída: criada a interface comum `AiEngine` em `src-tauri/src/engine.rs` implementada por `OllamaEngine` (`ollama.rs`).
