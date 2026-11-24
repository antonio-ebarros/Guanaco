# Guanaco

**Gerenciador de Inteligência Artificial local com interface de conversa integrada**, pensado para pessoas comuns testarem modelos de IA no próprio computador sem depender de terminal, Docker ou configurações técnicas.

## Por que escolher o Guanaco?

- **Ultraleve de verdade (Tauri + Rust):** ao contrário de clientes baseados em Electron/Chromium que consomem centenas de MB/GB de RAM só para abrir a janela, o Guanaco usa interface nativa leve para sobrar mais memória RAM e VRAM para o modelo de IA.
- **Simplicidade ("clicou, baixou, conversou"):** instala o motor local em 1 clique e recomenda modelos compatíveis com a memória RAM e GPU do seu PC, com tamanho em GB claro antes do download.
- **Proteção para PCs reais:** detecção realista de RAM/VRAM livre, reserva do sistema operacional, perfis **Conservador / Equilibrado** para não travar o computador e botão de **1 clique para liberar memória**.
- **Voz neural offline e Markdown ao vivo:** leitura em voz alta offline via **Piper TTS** (PT-BR, EN e ZH) e editor Markdown com formatação instantânea.
- **Privacidade absoluta:** 100% offline, sem conta, sem login e sem telemetria.
- **Multiplataforma:** empacotado com **Tauri v2** para **Windows** (`.exe` / `.msi`) e **Linux** (`.AppImage` / `.deb`), com verificador de atualizações integrado no aplicativo.

## Guanaco Pro (R$ 39 · Vitalício) — Apoie o projeto

O Guanaco é gratuito e de código aberto (Apache-2.0). Se quiser desbloquear recursos avançados **e apoiar o desenvolvimento independente do projeto**, você pode adquirir o **Guanaco Pro** com **pagamento único de R$ 39** (sem mensalidade e com ativação 100% offline via chave Ed25519):

- **Perfil Livre (`🔓 Livre`)** e **Parâmetros Avançados** (`temperature`, `top_p`, `repeat_penalty`, `num_ctx`, `num_gpu`).
- **Personalidades e System Prompt editável** para criar assistentes especializados.
- **Pasta de Conhecimento Local (RAG Offline):** converse com seus PDFs, DOCX, ODT e textos com citação das fontes.
- **Importador de modelos `.GGUF` locais** e suporte multi-motor (**Ollama** + **llamafile**).

## Estrutura do Projeto

```text
Guanaco/
├── src/                  # Frontend (HTML, CSS, JS sem dependências pesadas)
├── src-tauri/            # Backend nativo em Rust (hardware, processos, Ollama, storage)
├── docs/                 # Documentação técnica e decisões de arquitetura
├── .github/workflows/    # Build automático para Windows e Linux via GitHub Actions
├── CHANGELOG.md
└── README.md
```

## Como rodar em desenvolvimento (Linux)

### Pré-requisitos
```bash
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

### Executar
```bash
npm install
npm run tauri dev
```

### Gerar instalador local
```bash
npm run tauri build
```

Consulte [docs/ARQUITETURA.md](docs/ARQUITETURA.md) para detalhes técnicos do backend e da interface.

## Autor e Licença

Criado por **Antonio E. Barros** ([github.com/antonio-ebarros/Guanaco](https://github.com/antonio-ebarros/Guanaco)).

Distribuído sob a licença **Apache-2.0**. Consulte o arquivo [LICENSE](LICENSE) para mais detalhes.

## Créditos e Vozes Offline

- **Voz em Português do Brasil (`pt_BR-edresson-low`):** baseada no conjunto de dados aberto [TTS-Portuguese-Corpus](https://github.com/Edresson/TTS-Portuguese-Corpus) criado por **Edresson Casanova** (licença **CC BY 4.0**).
- **Sintetizador neural offline:** [Piper TTS](https://github.com/rhasspy/piper) (licença **MIT**).
- **Motor local padrão:** [Ollama](https://ollama.com) (licença **MIT**).
