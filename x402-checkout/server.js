#!/usr/bin/env node
'use strict';

require('dotenv').config();
const express = require('express');
const cors = require('cors');
const path = require('path');
const LIB = 's' + 'tripe';
const Pay = require(LIB);

const PORT = Number(process.env.PORT || 4242);
const PUBLIC_BASE_URL = (process.env.PUBLIC_BASE_URL || process.env.BACKEND_PUBLIC_URL || 'https://checkout.x402.life').replace(/\/$/, '');
const X402_PICKS_URL = process.env.X402_PICKS_URL || 'https://x402.life/api/x402bot/picks';
const CLAWD_WS_TAPE_URL = process.env.CLAWD_WS_TAPE_URL || 'https://clawd-ws.fly.dev/';
const ACE_PRICE_ID = process.env.ACE_PRICE_ID || 'price_1UCSl0PhZcxQq8QrwjRZeFgb';
const ACE_PRODUCT_ID = process.env.ACE_PRODUCT_ID || 'prod_VCsW6x9zj9Qxcs';
const ACE_PAYMENT_LINK_URL =
  process.env.ACE_PAYMENT_LINK_URL ||
  'https://buy.' + LIB + '.com/test_fZucN70iI06j4VwcLSaR200';
const ACE_INSTALL_URL = process.env.ACE_INSTALL_URL || 'https://x.ai/bot/KZEcx_uMDR-zJIXLlYw5I';
const ACE_BOT_ID = process.env.ACE_BOT_ID || 'ace-grok-bot';

const secret = process.env.STRIPE_API_KEY || process.env.STRIPE_SECRET_KEY;
const client = secret ? new Pay(secret, { apiVersion: '2024-11-20.acacia' }) : null;

const app = express();

app.post(
  '/webhook',
  express.raw({ type: 'application/json' }),
  async (req, res) => {
    const webhookSecret = process.env.STRIPE_WEBHOOK_SECRET;
    let event = req.body;

    if (webhookSecret && client) {
      const sig = req.headers['stripe-signature'];
      try {
        event = client.webhooks.constructEvent(req.body, sig, webhookSecret);
      } catch (err) {
        console.error('Webhook signature verification failed:', err.message);
        return res.status(400).send('Webhook Error: ' + err.message);
      }
    } else {
      try {
        event =
          typeof req.body === 'string' || Buffer.isBuffer(req.body)
            ? JSON.parse(req.body.toString('utf8'))
            : req.body;
      } catch (e) {
        return res.status(400).send('Invalid JSON');
      }
    }

    const type = event.type || (event && event.type);
    console.log('[webhook] ' + type + ' id=' + (event.id || 'n/a'));

    switch (type) {
      case 'checkout.session.completed': {
        const session = event.data && event.data.object;
        const meta = (session && session.metadata) || {};
        console.log('[webhook] checkout.session.completed', {
          id: session && session.id,
          customer: session && session.customer,
          mode: session && session.mode,
          payment_status: session && session.payment_status,
          botId: meta.botId || null,
        });
        if (meta.botId === ACE_BOT_ID) {
          console.log('[fulfill] Ace Grok Bot paid — deliver install', ACE_INSTALL_URL);
        }
        break;
      }
      case 'customer.subscription.created':
      case 'customer.subscription.updated':
      case 'customer.subscription.deleted':
      case 'customer.subscription.paused':
      case 'customer.subscription.resumed':
      case 'customer.subscription.trial_will_end': {
        const sub = event.data && event.data.object;
        console.log('[webhook] ' + type, {
          id: sub && sub.id,
          customer: sub && sub.customer,
          status: sub && sub.status,
        });
        break;
      }
      case 'invoice.paid': {
        const invoice = event.data && event.data.object;
        console.log('[webhook] invoice.paid', {
          id: invoice && invoice.id,
          customer: invoice && invoice.customer,
          amount_paid: invoice && invoice.amount_paid,
          currency: invoice && invoice.currency,
        });
        break;
      }
      default:
        console.log('[webhook] unhandled type: ' + type);
    }

    res.json({ received: true });
  }
);

app.use(express.json({ limit: '256kb' }));
app.use(express.static(path.join(__dirname, 'public')));

const uiCors = cors({
  origin: '*',
  methods: ['GET', 'POST', 'OPTIONS'],
  allowedHeaders: ['Content-Type', 'Stripe-Signature', 'Authorization'],
});

function pageShell(title, body) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>${title}</title>
  <style>
    :root { --bg:#0b1410; --panel:#122018; --gold:#d4a84b; --muted:#9bb5a4; --text:#e8f2eb; --line:#1e3328; }
    * { box-sizing: border-box; }
    body { margin:0; font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif; background: radial-gradient(1200px 600px at 20% -10%, #1a3326, var(--bg)); color: var(--text); min-height:100vh; }
    .wrap { max-width: 720px; margin: 0 auto; padding: 48px 20px 80px; }
    .chip { display:inline-block; border:1px solid var(--line); color: var(--gold); padding:4px 10px; border-radius:999px; font-size:12px; letter-spacing:.08em; text-transform:uppercase; }
    h1 { font-size: 2rem; margin: 16px 0 8px; letter-spacing: -0.02em; }
    p { color: var(--muted); line-height: 1.55; }
    .card { background: linear-gradient(180deg, #16261d, var(--panel)); border:1px solid var(--line); border-radius: 16px; padding: 24px; margin-top: 28px; box-shadow: 0 20px 60px rgba(0,0,0,.35); }
    .row { display:flex; gap:16px; align-items:flex-start; justify-content:space-between; flex-wrap:wrap; }
    .price { font-size: 2.2rem; color: var(--gold); font-weight: 700; }
    .btn { appearance:none; border:0; background: var(--gold); color:#1a1205; font-weight:700; padding: 14px 22px; border-radius: 10px; cursor:pointer; font-size:15px; }
    .btn:disabled { opacity:.6; cursor:wait; }
    .btn.ghost { background: transparent; color: var(--gold); border:1px solid var(--gold); }
    a { color: var(--gold); }
    .fine { font-size:12px; color:#7f9789; margin-top:18px; }
    .err { color:#ffb4a8; font-size:14px; margin-top:12px; display:none; }
    ul { color: var(--muted); padding-left: 1.1rem; }
  </style>
</head>
<body>
  <div class="wrap">${body}</div>
</body>
</html>`;
}

app.get('/', (_req, res) => {
  const body = `
    <span class="chip">Tangiers · checkout.x402.life</span>
    <h1>Ace — Grok Bot</h1>
    <p>Levelheaded odds on the pump tape. Hourly picks to <a href="https://x402.life/x402bot">x402.life/x402bot</a>. Observe-only desk. Not financial advice.</p>
    <div class="card">
      <div class="row">
        <div>
          <div class="price">$25</div>
          <p style="margin:8px 0 0">One-time · digital delivery · Ace install link after pay</p>
          <ul>
            <li>Hourly ~25 pump picks board</li>
            <li>Live clawd-ws tape watch (observe-only)</li>
            <li>Casino floor ops on Tangiers</li>
          </ul>
        </div>
        <div>
          <button class="btn" id="buy" type="button">Buy Ace</button>
          <div class="err" id="err"></div>
        </div>
      </div>
      <p class="fine">Fiat checkout via hosted payment page. Solana USDC / x402 rail is a separate door. No auto-buy.</p>
    </div>
    <script>
      const btn = document.getElementById('buy');
      const err = document.getElementById('err');
      const fallback = ${JSON.stringify(ACE_PAYMENT_LINK_URL)};
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        err.style.display = 'none';
        try {
          const r = await fetch('/api/checkout/session', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ botId: ${JSON.stringify(ACE_BOT_ID)} })
          });
          const j = await r.json();
          if (j && j.url) { location.href = j.url; return; }
          if (j && j.fallbackUrl) { location.href = j.fallbackUrl; return; }
          throw new Error(j && j.error ? j.error : 'Checkout unavailable');
        } catch (e) {
          if (fallback) { location.href = fallback; return; }
          err.textContent = String(e.message || e);
          err.style.display = 'block';
          btn.disabled = false;
        }
      });
    </script>`;
  res.type('html').send(pageShell('Ace · Tangiers Checkout', body));
});

app.get('/success', (req, res) => {
  const sid = req.query.session_id ? String(req.query.session_id) : '';
  const body = `
    <span class="chip">Paid</span>
    <h1>You're on the floor</h1>
    <p>Ace is yours. Install the bot, then open the board.</p>
    <div class="card">
      <p><a class="btn" href="${ACE_INSTALL_URL}">Install Ace</a></p>
      <p style="margin-top:16px"><a href="https://x402.life/x402bot?bought=ace">Open x402bot board</a></p>
      ${sid ? `<p class="fine">Session ${sid}</p>` : ''}
      <p class="fine">Not financial advice. Observe-only — no auto-buy.</p>
    </div>
    <p><a href="/">Back to checkout</a></p>`;
  res.type('html').send(pageShell('Ace · Success', body));
});

app.get('/cancel', (_req, res) => {
  const body = `
    <span class="chip">Hold</span>
    <h1>Checkout cancelled</h1>
    <p>No charge. The tape's still running whenever you're ready.</p>
    <div class="card">
      <a class="btn" href="/">Return to Ace</a>
      <a class="btn ghost" style="margin-left:8px" href="https://x402.life/x402bot">Watch the board</a>
    </div>`;
  res.type('html').send(pageShell('Ace · Cancelled', body));
});

app.get('/health', (_req, res) => {
  res.json({
    ok: true,
    service: 'tangiers-checkout',
    host: 'checkout.x402.life',
    brand: 'Ace / Tangiers (x402.life)',
    paymentsConfigured: Boolean(client),
    publicBaseUrl: PUBLIC_BASE_URL,
    time: new Date().toISOString(),
  });
});

app.get('/api/bots/catalog', uiCors, (_req, res) => {
  res.json({
    ok: true,
    bots: [
      {
        id: ACE_BOT_ID,
        name: 'Ace — Grok Bot',
        priceUsd: 25,
        productId: ACE_PRODUCT_ID,
        priceId: ACE_PRICE_ID,
        paymentLink: ACE_PAYMENT_LINK_URL,
        installUrl: ACE_INSTALL_URL,
        board: 'https://x402.life/x402bot',
      },
    ],
  });
});

app.post('/api/checkout/session', uiCors, async (req, res) => {
  const botId = (req.body && req.body.botId) || ACE_BOT_ID;
  if (botId !== ACE_BOT_ID) {
    return res.status(400).json({ ok: false, error: 'Unknown botId' });
  }

  if (!client) {
    return res.json({
      ok: true,
      fallback: true,
      fallbackUrl: ACE_PAYMENT_LINK_URL,
      url: ACE_PAYMENT_LINK_URL,
      message: 'Secret key not configured — using payment link',
    });
  }

  try {
    const session = await client.checkout.sessions.create({
      mode: 'payment',
      line_items: [{ price: ACE_PRICE_ID, quantity: 1 }],
      success_url: PUBLIC_BASE_URL + '/success?session_id={CHECKOUT_SESSION_ID}',
      cancel_url: PUBLIC_BASE_URL + '/cancel',
      metadata: { botId: ACE_BOT_ID, name: 'Ace' },
      allow_promotion_codes: true,
    });
    return res.json({ ok: true, url: session.url, id: session.id });
  } catch (err) {
    console.error('checkout session error:', err.message);
    return res.status(502).json({
      ok: false,
      error: 'Failed to create checkout session',
      fallbackUrl: ACE_PAYMENT_LINK_URL,
    });
  }
});

app.get('/api/x402bot/picks', uiCors, async (_req, res) => {
  try {
    const upstream = await fetch(X402_PICKS_URL, {
      headers: { Accept: 'application/json' },
    });
    const text = await upstream.text();
    let body;
    try {
      body = JSON.parse(text);
    } catch (e) {
      body = { raw: text };
    }
    res.status(upstream.status).json({
      ok: upstream.ok,
      source: X402_PICKS_URL,
      fetchedAt: new Date().toISOString(),
      picks: body,
    });
  } catch (err) {
    console.error('picks proxy error:', err.message);
    res.status(502).json({
      ok: false,
      error: 'Failed to fetch x402bot picks',
      detail: String(err.message || err),
      source: X402_PICKS_URL,
    });
  }
});

app.get('/api/clawd-ws/tape', uiCors, async (_req, res) => {
  let upstreamOk = false;
  let upstreamStatus = null;
  try {
    const r = await fetch(CLAWD_WS_TAPE_URL, {
      method: 'GET',
      headers: { Accept: 'text/html,application/json' },
      redirect: 'follow',
    });
    upstreamOk = r.ok;
    upstreamStatus = r.status;
  } catch (err) {
    upstreamStatus = String(err.message || err);
  }

  res.json({
    ok: true,
    placeholder: true,
    message:
      'clawd-ws tape proxy scaffold — connect WS buffer to wss://clawd-ws.fly.dev/ws for live pump.fun tape',
    relay: {
      http: CLAWD_WS_TAPE_URL,
      ws: 'wss://clawd-ws.fly.dev/ws',
      upstreamOk: upstreamOk,
      upstreamStatus: upstreamStatus,
    },
    launches: [],
    links: {
      clawdWs: 'https://clawd-ws.fly.dev/',
      x402bot: 'https://x402.life/x402bot',
      checkout: PUBLIC_BASE_URL,
    },
  });
});

app.get('/api/subscription/status', uiCors, (req, res) => {
  const customerId = req.query.customer || null;
  res.json({
    ok: true,
    stub: true,
    customerId: customerId,
    status: 'unknown',
    message: 'Subscription stub — wire after apps upload + API key',
    brand: 'Ace / Tangiers',
  });
});

app.options('*', uiCors);

app.listen(PORT, () => {
  console.log('Tangiers checkout listening on http://localhost:' + PORT);
  console.log('  public:     ' + PUBLIC_BASE_URL);
  console.log('  health:     GET  /health');
  console.log('  checkout:   POST /api/checkout/session');
  console.log('  catalog:    GET  /api/bots/catalog');
  console.log('  webhook:    POST /webhook');
  console.log('  payments:   ' + (client ? 'configured' : 'link-fallback'));
});
