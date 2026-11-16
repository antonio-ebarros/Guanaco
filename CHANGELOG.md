# Changelog

## [0.1.0] - Em desenvolvimento
- Estrutura inicial do projeto desktop multiplataforma (Windows e Linux) com **Tauri v2**.
- Documentação inicial (`README.md`, `docs/ARQUITETURA.md`) e pipeline de build em GitHub Actions (`.github/workflows/build.yml`).
- Detecção de hardware local (RAM, CPU, GPU) e painel de controle/transparência para modelos locais via Ollama.
- Interface de conversa limpa com editor Markdown ao vivo e catálogo guiado de modelos.
- Nova identidade visual com logo oficial `guanaco>_`, ícone do aplicativo 512x512, tema claro e tema escuro, efeito de vidro fosco na caixa de digitação e respostas com letra grande (`18px` por padrão) com controle rápido `A-`/`A+`.
- Recomendação inteligente de modelos baseada em RAM disponível no momento, VRAM disponível na GPU, núcleos da CPU, reserva do sistema operacional e modelos já carregados na memória.
