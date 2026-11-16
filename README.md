# Guanaco

**Gerenciador de Inteligência Artificial local com interface de conversa integrada**, pensado para pessoas comuns testarem modelos de IA no próprio computador sem depender de terminal, Docker ou configurações técnicas.

## Diferenciais

- **Simplicidade ("clicou, baixou, conversou"):** recomenda modelos compatíveis com a memória RAM e GPU do seu PC, com tamanho em GB claro antes do download.
- **Privacidade e Controle:** tudo roda localmente na sua máquina, com transparência sobre o que está rodando, quanto de memória RAM/VRAM está sendo usado, onde os arquivos ficam salvos e botão de **1 clique para liberar memória**.
- **Editor Markdown ao vivo:** escreva prompts estruturados com formatação instantânea (`#` título, `-` lista, `>` citação, `**negrito**`, `` `código` ``).
- **Multiplataforma:** empacotado com **Tauri v2** para **Windows** (`.exe` / `.msi`) e **Linux** (`.AppImage` / `.deb`).

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

## Licença

Distribuído sob a licença **Apache-2.0**. Consulte o arquivo [LICENSE](LICENSE) para mais detalhes.
