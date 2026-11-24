/**
 * Guanaco Pro — Worker de Entrega Automática (Cloudflare Workers + Mercado Pago + Resend)
 *
 * Fluxo:
 *   1. Recebe webhook do Mercado Pago (POST /webhook/mercadopago)
 *   2. Valida assinatura HMAC-SHA256 (x-signature) se MP_WEBHOOK_SECRET estiver configurado
 *   3. Consulta o pagamento na API do Mercado Pago e confirma status === 'approved'
 *   4. Gera a chave vitalícia offline Ed25519 (GUANACO-PRO-...) usando GUANACO_PRO_PRIVATE_PEM
 *   5. Envia a chave por e-mail ao comprador via Resend
 */

function toBase64Url(bytes) {
  let binary = '';
  const arr = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  for (let i = 0; i < arr.length; i++) {
    binary += String.fromCharCode(arr[i]);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function pemToArrayBuffer(pem) {
  const b64 = pem
    .replace(/-----BEGIN PRIVATE KEY-----/g, '')
    .replace(/-----END PRIVATE KEY-----/g, '')
    .replace(/\s+/g, '');
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

async function generateGuanacoProKey({ holder, email, privatePem }) {
  const keyData = pemToArrayBuffer(privatePem);
  const cryptoKey = await crypto.subtle.importKey(
    'pkcs8',
    keyData,
    { name: 'Ed25519' },
    false,
    ['sign']
  );

  const payloadObj = {
    holder: holder || 'Guanaco Pro User',
    email: email || '',
    tier: 'pro',
    issuedAt: new Date().toISOString().slice(0, 10),
  };

  const encoder = new TextEncoder();
  const payloadB64 = toBase64Url(encoder.encode(JSON.stringify(payloadObj)));
  const sigBuf = await crypto.subtle.sign('Ed25519', cryptoKey, encoder.encode(payloadB64));
  const sigB64 = toBase64Url(sigBuf);

  return `GUANACO-PRO-${payloadB64}.${sigB64}`;
}

async function verifyMercadoPagoSignature(request, dataId, secret) {
  if (!secret) return true;
  const xSignature = request.headers.get('x-signature') || '';
  const xRequestId = request.headers.get('x-request-id') || '';

  const parts = {};
  for (const part of xSignature.split(',')) {
    const [k, v] = part.split('=').map(s => s?.trim());
    if (k && v) parts[k] = v;
  }
  if (!parts.ts || !parts.v1) return false;

  const manifest = `id:${dataId};request-id:${xRequestId};ts:${parts.ts};`;
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );
  const sig = await crypto.subtle.sign('HMAC', key, encoder.encode(manifest));
  const hex = Array.from(new Uint8Array(sig))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');

  return hex === parts.v1;
}

function parseSender(rawFrom) {
  const raw = String(rawFrom || '').trim();
  const match = raw.match(/^(.*)<([^>]+)>$/);
  if (match) {
    return {
      name: match[1].trim().replace(/^["']|["']$/g, '') || 'Guanaco Pro',
      email: match[2].trim(),
    };
  }
  return { name: 'Guanaco Pro', email: raw };
}

async function sendLicenseEmail({ env, to, holder, licenseKey }) {
  const subject = 'Sua licença do Guanaco Pro chegou!';
  const textBody = [
    `Olá, ${holder}!`,
    '',
    'Obrigado por apoiar o Guanaco! Aqui está a sua chave de licença vitalícia (100% offline):',
    '',
    licenseKey,
    '',
    'Como ativar:',
    '1. Abra o Guanaco no seu computador.',
    '2. Vá na aba "Sobre / Guanaco Pro".',
    '3. Cole a chave acima no campo de ativação e clique em "Ativar Licença Pro".',
    '',
    'Guarde este e-mail caso precise ativar novamente no futuro.',
    'Qualquer dúvida, basta responder a este e-mail.',
  ].join('\n');

  if (env.BREVO_API_KEY) {
    const sender = parseSender(env.EMAIL_FROM);
    const res = await fetch('https://api.brevo.com/v3/smtp/email', {
      method: 'POST',
      headers: {
        'api-key': env.BREVO_API_KEY,
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify({
        sender,
        to: [{ email: to, name: holder }],
        subject,
        textContent: textBody,
      }),
    });
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`Erro ao enviar e-mail via Brevo (${res.status}): ${body}`);
    }
    return;
  }

  if (env.RESEND_API_KEY) {
    const res = await fetch('https://api.resend.com/emails', {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${env.RESEND_API_KEY}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        from: env.EMAIL_FROM,
        to: [to],
        subject,
        text: textBody,
      }),
    });
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`Erro ao enviar e-mail via Resend (${res.status}): ${body}`);
    }
    return;
  }

  if (!env.BREVO_API_KEY && !env.RESEND_API_KEY) {
    return;
  }
}

async function createCheckoutPreference(origin, env) {
  const unitPrice = Number(env.GUANACO_PRO_PRICE_BRL || 39);
  const res = await fetch('https://api.mercadopago.com/checkout/preferences', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.MP_ACCESS_TOKEN}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      items: [
        {
          title: 'Guanaco Pro — Licença Vitalícia',
          quantity: 1,
          currency_id: 'BRL',
          unit_price: unitPrice,
        },
      ],
      notification_url: `${origin}/webhook/mercadopago`,
      back_urls: {
        success: `${origin}/obrigado`,
        pending: `${origin}/obrigado?status=pending`,
        failure: `${origin}/comprar`,
      },
      auto_return: 'approved',
      statement_descriptor: 'GUANACO PRO',
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Erro ao criar Checkout Pro (${res.status}): ${body}`);
  }
  const data = await res.json();
  if (!data?.init_point) {
    throw new Error('Mercado Pago não retornou init_point.');
  }
  return data.init_point;
}

function escapeHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function thankYouHtml({ status, holder, licenseKey }) {
  const isApproved = status === 'approved' && Boolean(licenseKey);
  const isPending = status === 'pending' || status === 'in_process';

  let bodyHtml = '';
  if (isApproved) {
    const safeKey = escapeHtml(licenseKey);
    const safeHolder = escapeHtml(holder || 'Apoiador');
    bodyHtml = `
      <h1>Obrigado, ${safeHolder}!</h1>
      <p><strong>Pagamento aprovado!</strong> Aqui está a sua chave vitalícia do Guanaco Pro (100% offline):</p>
      <div class="key-box" id="license-key">${safeKey}</div>
      <button class="copy-btn" type="button" onclick="navigator.clipboard.writeText(document.getElementById('license-key').textContent).then(()=>{this.textContent='✓ Chave copiada!';})">📋 Copiar chave</button>
      <p class="steps">
        <strong>Como ativar:</strong><br>
        1. Abra o Guanaco no seu computador.<br>
        2. Vá em <strong>Configurações → Guanaco Pro</strong>.<br>
        3. Cole a chave acima e clique em <strong>Ativar Licença Pro</strong>.
      </p>
      <p class="warn">Guarde uma cópia desta chave em um bloco de notas ou gerenciador de senhas caso formate seu PC.</p>
    `;
  } else if (isPending) {
    bodyHtml = `
      <h1>Aguardando confirmação…</h1>
      <p>Estamos aguardando a confirmação do seu pagamento (Pix ou boleto). Esta página atualiza automaticamente em alguns segundos.</p>
      <button class="copy-btn" type="button" onclick="location.reload()">🔄 Verificar agora</button>
      <script>setTimeout(() => location.reload(), 4000);</script>
    `;
  } else {
    bodyHtml = `
      <h1>Obrigado por apoiar o Guanaco!</h1>
      <p>Não foi possível localizar um pagamento aprovado nesta URL.</p>
      <p>Se você acabou de pagar e esta tela não exibiu sua chave, entre em contato informando o nome e e-mail da compra no Mercado Pago.</p>
    `;
  }

  return `<!doctype html>
<html lang="pt-BR">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Guanaco Pro — Licença</title>
  <style>
    body{font-family:system-ui,-apple-system,sans-serif;background:#0f172a;color:#e2e8f0;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0;padding:20px}
    .card{max-width:560px;width:100%;background:#1e293b;border:1px solid #334155;border-radius:16px;padding:28px;text-align:center;line-height:1.6}
    h1{margin:0 0 8px;font-size:22px;color:#f8fafc}
    p{margin:10px 0;color:#cbd5e1;font-size:15px}
    .tag{display:inline-block;padding:4px 12px;border-radius:999px;background:rgba(245,158,11,.15);color:#fbbf24;font-weight:700;font-size:12px;margin-bottom:12px}
    .key-box{margin:14px 0;padding:12px;border-radius:10px;background:#0f172a;border:1px solid #475569;color:#fbbf24;font-family:ui-monospace,monospace;font-size:12.5px;word-break:break-all;user-select:all}
    .copy-btn{padding:10px 18px;border:0;border-radius:10px;background:#f59e0b;color:#0f172a;font-weight:700;font-size:14px;cursor:pointer}
    .copy-btn:hover{filter:brightness(1.08)}
    .steps{text-align:left;background:rgba(15,23,42,.5);padding:12px 16px;border-radius:10px;font-size:14px;margin-top:16px}
    .warn{font-size:13px;color:#94a3b8;margin-top:12px}
  </style>
</head>
<body>
  <div class="card">
    <span class="tag">✨ GUANACO PRO</span>
    ${bodyHtml}
  </div>
</body>
</html>`;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method === 'GET' && url.pathname === '/health') {
      return new Response('ok', { status: 200 });
    }

    if (request.method === 'GET' && url.pathname === '/comprar') {
      try {
        const checkoutUrl = await createCheckoutPreference(url.origin, env);
        return Response.redirect(checkoutUrl, 302);
      } catch (err) {
        return new Response(`Erro ao iniciar pagamento: ${err.message}`, { status: 502 });
      }
    }

    if (request.method === 'GET' && url.pathname === '/obrigado') {
      const paymentId = url.searchParams.get('payment_id') || url.searchParams.get('collection_id');
      const fallbackStatus = url.searchParams.get('status') || url.searchParams.get('collection_status') || '';

      if (!paymentId) {
        return new Response(thankYouHtml({ status: fallbackStatus }), {
          status: 200,
          headers: { 'Content-Type': 'text/html; charset=utf-8' },
        });
      }

      const mpRes = await fetch(`https://api.mercadopago.com/v1/payments/${encodeURIComponent(paymentId)}`, {
        headers: { Authorization: `Bearer ${env.MP_ACCESS_TOKEN}` },
      });
      if (!mpRes.ok) {
        return new Response(thankYouHtml({ status: fallbackStatus }), {
          status: 200,
          headers: { 'Content-Type': 'text/html; charset=utf-8' },
        });
      }

      const payment = await mpRes.json();
      if (payment.status !== 'approved') {
        return new Response(thankYouHtml({ status: payment.status }), {
          status: 200,
          headers: { 'Content-Type': 'text/html; charset=utf-8' },
        });
      }

      const email = payment?.payer?.email || '';
      const first = payment?.payer?.first_name || '';
      const last = payment?.payer?.last_name || '';
      const holder = `${first} ${last}`.trim() || email.split('@')[0] || 'Guanaco Pro User';

      const licenseKey = await generateGuanacoProKey({
        holder,
        email,
        privatePem: env.GUANACO_PRO_PRIVATE_PEM,
      });

      return new Response(thankYouHtml({ status: 'approved', holder, licenseKey }), {
        status: 200,
        headers: { 'Content-Type': 'text/html; charset=utf-8' },
      });
    }

    if (request.method !== 'POST' || url.pathname !== '/webhook/mercadopago') {
      return new Response('Not found', { status: 404 });
    }

    let body = {};
    try {
      body = await request.json();
    } catch {
      return new Response('Invalid JSON', { status: 400 });
    }

    const paymentId = body?.data?.id || url.searchParams.get('data.id');
    const eventType = body?.type || body?.topic || url.searchParams.get('topic');

    if (eventType !== 'payment' || !paymentId) {
      return new Response('Ignored event', { status: 200 });
    }

    const validSig = await verifyMercadoPagoSignature(request, paymentId, env.MP_WEBHOOK_SECRET);
    if (!validSig) {
      return new Response('Invalid signature', { status: 401 });
    }

    const mpRes = await fetch(`https://api.mercadopago.com/v1/payments/${encodeURIComponent(paymentId)}`, {
      headers: { Authorization: `Bearer ${env.MP_ACCESS_TOKEN}` },
    });
    if (!mpRes.ok) {
      return new Response('Failed to fetch payment', { status: 502 });
    }

    const payment = await mpRes.json();
    if (payment.status !== 'approved') {
      return new Response(`Payment status: ${payment.status}`, { status: 200 });
    }

    const email = payment?.payer?.email || '';
    if (!email) {
      return new Response('Missing payer email', { status: 200 });
    }

    const first = payment?.payer?.first_name || '';
    const last = payment?.payer?.last_name || '';
    const holder = `${first} ${last}`.trim() || email.split('@')[0];

    const licenseKey = await generateGuanacoProKey({
      holder,
      email,
      privatePem: env.GUANACO_PRO_PRIVATE_PEM,
    });

    await sendLicenseEmail({
      env,
      to: email,
      holder,
      licenseKey,
    });

    return new Response('License delivered', { status: 200 });
  },
};
