# Guanaco Pro — Entrega Automática (Mercado Pago + Cloudflare Workers)

Estrutura pronta para receber pagamentos via **Checkout Pro do Mercado Pago**, gerar a chave vitalícia offline (`GUANACO-PRO-...`) em **Ed25519** e **exibir a chave instantaneamente na tela com botão de copiar** após a aprovação — **sem precisar de serviço de e-mail nem domínio próprio**, com custo fixo **R\$ 0,00**.

---

## Pré-requisitos

1. Conta gratuita na [Cloudflare](https://dash.cloudflare.com/).
2. Conta no **Mercado Pago Developers** (`mercadopago.com.br/developers`) com o `Access Token` de produção.

---

## Como publicar

Entre nesta pasta e faça login na Cloudflare:

```bash
cd tools/license-webhook
npx wrangler login
```

Cadastre apenas os **2 segredos obrigatórios** (criptografados na Cloudflare):

```bash
# 1. Chave privada Ed25519 (lê direto do arquivo, sem erro de quebra de linha)
npx wrangler secret put GUANACO_PRO_PRIVATE_PEM < ../../.keys/guanaco_pro_private.pem

# 2. Token de produção do Mercado Pago
npx wrangler secret put MP_ACCESS_TOKEN
```

*(Opcional: se no futuro quiser também enviar uma cópia por e-mail via Brevo/Resend, basta cadastrar `BREVO_API_KEY` + `EMAIL_FROM` + `MP_WEBHOOK_SECRET`.)*

Publique o Worker:

```bash
npx wrangler deploy
```

A Cloudflare vai gerar uma URL como `https://guanaco-license-webhook.<seu-subdominio>.workers.dev`.

---

## Rotas disponíveis no Worker

- `GET /comprar` — cria uma sessão do **Checkout Pro** no Mercado Pago (R$ 39,00) e redireciona o comprador direto para a tela de pagamento.
- `GET /obrigado` — página de confirmação exibida após o pagamento.
- `POST /webhook/mercadopago` — recebe a notificação de pagamento aprovado, gera a chave `GUANACO-PRO-...` e envia o e-mail.

> **Preço configurável:** por padrão o valor é `39` (R$ 39,00). Para mudar sem editar código, defina a variável `GUANACO_PRO_PRICE_BRL` no painel da Cloudflare ou em `wrangler.toml`.

---

## Configurar no app e no Mercado Pago

1. Em `src/js/pro.js`, atualize `GUANACO_PRO_CHECKOUT_URL` para:
   `https://guanaco-license-webhook.<seu-subdominio>.workers.dev/comprar`
2. No painel de desenvolvedor do Mercado Pago, vá em **Webhooks**:
   - Adicione a URL: `https://guanaco-license-webhook.<seu-subdominio>.workers.dev/webhook/mercadopago`
   - Marque o evento **Pagamentos (`payment`)**.
   - Copie a chave secreta gerada e salve em `MP_WEBHOOK_SECRET`.
