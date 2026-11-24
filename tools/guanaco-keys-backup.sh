#!/usr/bin/env sh
# ==============================================================================
# Guanaco Pro — Backup e Restauração Segura das Chaves (Ed25519)
# Compatível com qualquer distribuição Linux (POSIX sh + tar + openssl).
#
# O que é salvo:
#   - .keys/guanaco_pro_private.pem  (chave privada — crítica!)
#   - .keys/guanaco_pro_public.json   (chave pública de conferência)
#
# Segurança:
#   - Arquivo compactado e criptografado com AES-256-CBC + PBKDF2 (600.000 iterações)
#   - Permissões restritas (umask 077 / chmod 600)
#   - Nenhum arquivo temporário descriptografado permanece no disco
#
# Uso:
#   ./tools/guanaco-keys-backup.sh backup  <pasta_destino>
#   ./tools/guanaco-keys-backup.sh verify  <arquivo_backup.tar.gz.enc>
#   ./tools/guanaco-keys-backup.sh restore <arquivo_backup.tar.gz.enc>
# ==============================================================================

set -eu
umask 077

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
KEYS_DIR="$ROOT_DIR/.keys"
PRIV_FILE="guanaco_pro_private.pem"
PUB_FILE="guanaco_pro_public.json"

TMP_DIR=""

cleanup() {
  if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
    if command -v shred >/dev/null 2>&1; then
      find "$TMP_DIR" -type f -exec shred -u {} + 2>/dev/null || true
    fi
    rm -rf "$TMP_DIR"
  fi
}

trap cleanup EXIT INT TERM

die() {
  printf 'Erro: %s\n' "$1" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Comando obrigatório não encontrado: $1"
}

check_deps() {
  require_cmd tar
  require_cmd gzip
  require_cmd openssl
  require_cmd mktemp
  require_cmd chmod
}

calc_sha256() {
  target_file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$target_file" | awk '{print $1}'
  else
    openssl dgst -sha256 "$target_file" | awk '{print $NF}'
  fi
}

usage() {
  cat <<EOF
Uso:
  $0 backup  [pasta_destino]              Cria um backup criptografado em <pasta_destino>
  $0 verify  <arquivo.tar.gz.enc>         Testa a integridade e a senha do backup sem alterar .keys/
  $0 restore <arquivo.tar.gz.enc>         Restaura as chaves para .keys/ com permissões seguras

Exemplos:
  $0 backup /media/\$USER/PENDRIVE/guanaco-backup
  $0 verify /media/\$USER/PENDRIVE/guanaco-backup/guanaco-keys-backup-20261124-165000.tar.gz.enc
  $0 restore /media/\$USER/PENDRIVE/guanaco-backup/guanaco-keys-backup-20261124-165000.tar.gz.enc
EOF
}

cmd_backup() {
  dest_dir="${1:-}"
  if [ -z "$dest_dir" ]; then
    printf 'Digite a pasta de destino para salvar o backup: '
    read -r dest_dir
  fi
  [ -n "$dest_dir" ] || die "Pasta de destino não informada."

  [ -f "$KEYS_DIR/$PRIV_FILE" ] || die "Chave privada não encontrada em $KEYS_DIR/$PRIV_FILE"
  [ -f "$KEYS_DIR/$PUB_FILE" ] || die "Chave pública não encontrada em $KEYS_DIR/$PUB_FILE"

  mkdir -p "$dest_dir" || die "Não foi possível criar a pasta de destino: $dest_dir"
  dest_dir=$(CDPATH= cd -- "$dest_dir" && pwd)

  timestamp=$(date +%Y%m%d-%H%M%S)
  out_file="$dest_dir/guanaco-keys-backup-$timestamp.tar.gz.enc"
  sha_file="$out_file.sha256"

  TMP_DIR=$(mktemp -d)
  stage_dir="$TMP_DIR/stage"
  mkdir -p "$stage_dir"

  cp "$KEYS_DIR/$PRIV_FILE" "$stage_dir/$PRIV_FILE"
  cp "$KEYS_DIR/$PUB_FILE" "$stage_dir/$PUB_FILE"
  chmod 600 "$stage_dir/$PRIV_FILE"
  chmod 644 "$stage_dir/$PUB_FILE"

  {
    printf 'Guanaco Pro Keys Backup\n'
    printf 'Data (UTC): %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'Host: %s\n' "$(uname -n)"
    printf 'SHA256 %s: %s\n' "$PRIV_FILE" "$(calc_sha256 "$stage_dir/$PRIV_FILE")"
    printf 'SHA256 %s: %s\n' "$PUB_FILE" "$(calc_sha256 "$stage_dir/$PUB_FILE")"
  } > "$stage_dir/MANIFEST.txt"
  chmod 600 "$stage_dir/MANIFEST.txt"

  printf 'Defina uma senha forte para criptografar o backup (será pedida duas vezes):\n'
  tar -C "$stage_dir" -czf - "$PRIV_FILE" "$PUB_FILE" "MANIFEST.txt" \
    | openssl enc -aes-256-cbc -pbkdf2 -iter 600000 -salt -out "$out_file"

  chmod 600 "$out_file"
  hash_val=$(calc_sha256 "$out_file")
  printf '%s  %s\n' "$hash_val" "$(basename "$out_file")" > "$sha_file"
  chmod 600 "$sha_file"

  printf '\nBackup criptografado criado com sucesso!\n'
  printf '  Arquivo : %s\n' "$out_file"
  printf '  Checksum: %s\n' "$sha_file"
  printf '\nDica: rode "%s verify %s" para confirmar que a senha e o arquivo estão 100%% ok.\n' "$0" "$out_file"
}

verify_checksum_if_present() {
  enc_file="$1"
  sha_file="$enc_file.sha256"
  if [ -f "$sha_file" ]; then
    expected=$(awk '{print $1}' "$sha_file")
    actual=$(calc_sha256 "$enc_file")
    [ "$expected" = "$actual" ] || die "Falha na verificação de integridade (.sha256 não confere). O arquivo pode estar corrompido."
    printf 'Integridade SHA-256 verificada com sucesso.\n'
  else
    printf 'Aviso: arquivo .sha256 não encontrado; seguindo apenas com a validação criptográfica.\n'
  fi
}

decrypt_to_temp() {
  enc_file="$1"
  [ -f "$enc_file" ] || die "Arquivo de backup não encontrado: $enc_file"
  verify_checksum_if_present "$enc_file"

  TMP_DIR=$(mktemp -d)
  extract_dir="$TMP_DIR/extracted"
  mkdir -p "$extract_dir"

  printf 'Digite a senha do backup para descriptografar:\n'
  openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 -in "$enc_file" \
    | tar -xzf - -C "$extract_dir" \
    || die "Falha ao descriptografar ou extrair (senha incorreta ou arquivo corrompido)."

  [ -s "$extract_dir/$PRIV_FILE" ] || die "Arquivo inválido: $PRIV_FILE ausente ou vazio."
  [ -s "$extract_dir/$PUB_FILE" ] || die "Arquivo inválido: $PUB_FILE ausente ou vazio."
}

cmd_verify() {
  enc_file="${1:-}"
  [ -n "$enc_file" ] || die "Informe o caminho do arquivo .tar.gz.enc"
  decrypt_to_temp "$enc_file"

  extract_dir="$TMP_DIR/extracted"
  printf '\nBackup válido e senha confirmada!\n'
  if [ -f "$extract_dir/MANIFEST.txt" ]; then
    printf '%s\n' '--- Manifesto ---'
    cat "$extract_dir/MANIFEST.txt"
    printf '%s\n' '-----------------'
  fi
}

cmd_restore() {
  enc_file="${1:-}"
  [ -n "$enc_file" ] || die "Informe o caminho do arquivo .tar.gz.enc"

  if [ -f "$KEYS_DIR/$PRIV_FILE" ]; then
    printf 'ATENÇÃO: já existe uma chave privada em %s\n' "$KEYS_DIR/$PRIV_FILE"
    printf 'Deseja sobrescrever com o backup? (s/N): '
    read -r ans
    case "$ans" in
      s|S|sim|SIM|y|Y|yes|YES) ;;
      *)
        printf 'Restauração cancelada. Nenhuma chave foi alterada.\n'
        exit 0
        ;;
    esac
  fi

  decrypt_to_temp "$enc_file"
  extract_dir="$TMP_DIR/extracted"

  mkdir -p "$KEYS_DIR"
  chmod 700 "$KEYS_DIR"

  cp "$extract_dir/$PRIV_FILE" "$KEYS_DIR/$PRIV_FILE"
  cp "$extract_dir/$PUB_FILE" "$KEYS_DIR/$PUB_FILE"
  chmod 600 "$KEYS_DIR/$PRIV_FILE"
  chmod 644 "$KEYS_DIR/$PUB_FILE"

  if [ -f "$ROOT_DIR/.gitignore" ] && ! grep -qxF '.keys/' "$ROOT_DIR/.gitignore"; then
    printf '\nAVISO: ".keys/" não foi encontrado no .gitignore! Verifique antes de fazer commit.\n' >&2
  fi

  printf '\nChaves restauradas com sucesso em %s\n' "$KEYS_DIR"
  printf '  %s (chmod 600)\n' "$PRIV_FILE"
  printf '  %s (chmod 644)\n' "$PUB_FILE"
}

check_deps

action="${1:-}"
case "$action" in
  backup)
    shift
    cmd_backup "${1:-}"
    ;;
  verify)
    shift
    cmd_verify "${1:-}"
    ;;
  restore)
    shift
    cmd_restore "${1:-}"
    ;;
  -h|--help|help|"")
    usage
    exit 0
    ;;
  *)
    usage >&2
    die "Ação desconhecida: $action"
    ;;
esac
