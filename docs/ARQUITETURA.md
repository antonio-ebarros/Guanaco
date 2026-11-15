# Arquitetura do Guanaco

## Visão Geral

O Guanaco é dividido em duas camadas leves:

1. **Frontend (`src/`):**
   - HTML/CSS/JavaScript puro (sem frameworks pesados para economizar RAM, já que modelos de IA consomem bastante memória).
   - Três visões principais acessíveis na barra lateral:
     - **Conversa:** chat com streaming, anexos de texto/imagem e editor Markdown ao vivo.
     - **Modelos:** catálogo traduzido para leigos (Leve, Equilibrado, Capaz, Visão), com selo de compatibilidade baseado no hardware detectado e barra de progresso de download.
     - **Controle & Transparência:** mostra status do motor local (`127.0.0.1:11434`), uso de RAM/VRAM por modelo ativo, botão para descarregar da memória, espaço em disco ocupado e localização dos arquivos.

2. **Backend Nativo (`src-tauri/src/`):**
   - Escrito em **Rust** com **Tauri v2**.
   - **Diagnóstico de Hardware (`sysinfo` + `nvidia-smi`):** lê RAM total/disponível, quantidade de núcleos de CPU e presença de GPU dedicada.
   - **Gerenciamento do Motor (Ollama):** verifica se o serviço local está ativo em `http://127.0.0.1:11434`, inicia `ollama serve` com 1 clique quando instalado e consulta `/api/tags`, `/api/ps`, `/api/pull`, `/api/delete` e `/api/chat`.
   - **Persistência Local:** conversas e preferências ficam salvas em JSON na pasta de dados do aplicativo (`app_data_dir`).
