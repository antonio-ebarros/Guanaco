# Arquitetura do Guanaco

## Visão Geral

O Guanaco é dividido em duas camadas leves:

1. **Frontend (`src/`):**
   - HTML/CSS/JavaScript modular (`ES Modules`, sem frameworks pesados para economizar RAM):
     - `app.js`: orquestrador principal e listeners de eventos do Tauri.
     - `js/state.js`: estado compartilhado, presets de modelos e utilitários.
     - `js/theme.js`: temas claro/escuro, tamanho da fonte e Modo Econômico.
     - `js/models.js`: detecção de hardware, catálogo de modelos e painel de controle/RAM.
     - `js/chat.js`: histórico de conversas, anexos, editor Markdown ao vivo e streaming.
     - `js/speech.js`: leitura em voz alta (`Ouvir`) e cópia rápida (`Copiar`) das respostas.
   - Três visões principais acessíveis na barra lateral:
     - **Conversa:** chat com streaming, anexos de texto/imagem e editor Markdown ao vivo.
     - **Modelos:** catálogo traduzido para leigos (Leve, Equilibrado, Capaz, Visão), com selo de compatibilidade baseado no hardware detectado e barra de progresso de download.
     - **Controle & Transparência:** mostra status do motor local (`127.0.0.1:11434`), uso de RAM/VRAM por modelo ativo, botão para descarregar da memória, espaço em disco ocupado e localização dos arquivos.

2. **Backend Nativo (`src-tauri/src/`):**
   - Escrito em **Rust** com **Tauri v2**, organizado em módulos:
     - `lib.rs`: inicialização do Tauri, estado compartilhado e registro dos comandos.
     - `engine.rs` (**Interface `AiEngine`**): abstração comum de motor de IA local, permitindo acoplar novos motores (`llamafile`, `llama.cpp`) no futuro sem alterar a UI.
     - `hardware.rs` (**Diagnóstico de Hardware**): lê RAM total/disponível, reserva do SO, núcleos de CPU e VRAM livre/total da GPU (`sysinfo` + `nvidia-smi`).
     - `ollama.rs` (**Gerenciamento do Motor**): instala o Ollama automaticamente em 1 clique, inicia `ollama serve` localmente (`http://127.0.0.1:11434`), controla streaming/cancelamento, Modo Econômico e limpeza ao sair.
     - `storage.rs` (**Persistência Local**): grava e restaura conversas em JSON de forma atômica com backup automático (`conversations.bak.json`) em `app_data_dir`.
