# checkout.x402.life

Ace Grok Bot fiat checkout (Tangiers).

## Local

```bash
cd x402-checkout
cp .env.example .env
# fill STRIPE_API_KEY
npm install
PUBLIC_BASE_URL=http://localhost:4242 npm start
```

The process listens on `PORT` (default `4242`). Do not pin `PORT` when deploying — Railway injects it.

## Railway

Create a **fresh** project for this app (do not reuse `clawd-tee-gateway`).

```bash
cd x402-checkout
railway init --name x402-checkout
railway add --service x402-checkout --json
```

Set service variables from local `.env` (never commit `.env`). Do **not** set `PORT`.

```bash
printf '%s' "$STRIPE_API_KEY" | railway variable set STRIPE_API_KEY --stdin --service x402-checkout
railway variable set PUBLIC_BASE_URL=https://checkout.x402.life --service x402-checkout
railway variable set ACE_PRICE_ID=price_1UCSl0PhZcxQq8QrwjRZeFgb --service x402-checkout
railway variable set ACE_PRODUCT_ID=prod_VCsW6x9zj9Qxcs --service x402-checkout
railway variable set ACE_BOT_ID=ace-grok-bot --service x402-checkout
railway variable set ACE_INSTALL_URL=https://x.ai/bot/KZEcx_uMDR-zJIXLlYw5I --service x402-checkout
railway variable set X402_PICKS_URL=https://x402.life/api/x402bot/picks --service x402-checkout
```

Deploy from this directory and generate a public `*.up.railway.app` domain:

```bash
railway up --detach -m "deploy tangiers checkout"
railway domain --service x402-checkout --json
```

Current Railway service domain: https://x402-checkout-production.up.railway.app

Optional custom DNS (Cloudflare change is separate): CNAME `checkout` → `x402-checkout-production.up.railway.app`.

Webhook URL until DNS is pointed: `https://x402-checkout-production.up.railway.app/webhook`.

## Webhook

https://checkout.x402.life/webhook

Events: `checkout.session.completed`, `customer.subscription.*`, `invoice.paid`

## Product

`prod_VCsW6x9zj9Qxcs` / `price_1UCSl0PhZcxQq8QrwjRZeFgb` / `plink_1UCSl8PhZcxQq8QrPucBIJMf`
