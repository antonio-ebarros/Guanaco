# Changelog

## [0.1.0] - Em desenvolvimento
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
