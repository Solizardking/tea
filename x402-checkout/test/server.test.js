'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const http = require('node:http');
const net = require('node:net');
const path = require('node:path');

const ROOT = path.join(__dirname, '..');
const SERVER = path.join(ROOT, 'server.js');

function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address();
      const port = typeof addr === 'object' && addr ? addr.port : 0;
      server.close((err) => (err ? reject(err) : resolve(port)));
    });
    server.on('error', reject);
  });
}

function request(port, method, urlPath, body) {
  return new Promise((resolve, reject) => {
    const data = body === undefined ? null : JSON.stringify(body);
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port,
        path: urlPath,
        method,
        headers: data
          ? {
              'Content-Type': 'application/json',
              'Content-Length': Buffer.byteLength(data),
            }
          : {},
      },
      (res) => {
        let chunks = '';
        res.on('data', (c) => {
          chunks += c;
        });
        res.on('end', () => {
          let json = null;
          try {
            json = JSON.parse(chunks);
          } catch {
            json = null;
          }
          resolve({ status: res.statusCode, body: chunks, json });
        });
      }
    );
    req.on('error', reject);
    if (data) req.write(data);
    req.end();
  });
}

function waitForHealth(port, timeoutMs = 10000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const attempt = () => {
      request(port, 'GET', '/health')
        .then((res) => {
          if (res.json && res.json.ok === true) {
            resolve(res);
            return;
          }
          retry();
        })
        .catch(retry);
    };
    const retry = () => {
      if (Date.now() - started > timeoutMs) {
        reject(new Error('timeout waiting for shipped server on port ' + port));
        return;
      }
      setTimeout(attempt, 50);
    };
    attempt();
  });
}

function startServer() {
  return getFreePort().then((port) => {
    const publicBase = 'http://127.0.0.1:' + port;
    const child = spawn(process.execPath, [SERVER], {
      cwd: ROOT,
      env: {
        ...process.env,
        PORT: String(port),
        PUBLIC_BASE_URL: publicBase,
        STRIPE_API_KEY: '',
        STRIPE_SECRET_KEY: '',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => {
      stdout += d;
    });
    child.stderr.on('data', (d) => {
      stderr += d;
    });
    return waitForHealth(port).then(
      () => ({ child, port, publicBase, stdout, stderr }),
      (err) => {
        child.kill('SIGKILL');
        return Promise.reject(
          new Error(err.message + '\nstdout:\n' + stdout + '\nstderr:\n' + stderr)
        );
      }
    );
  });
}

function stopServer(child) {
  return new Promise((resolve) => {
    if (!child || child.exitCode !== null || child.killed) {
      resolve();
      return;
    }
    const done = () => resolve();
    child.once('exit', done);
    child.kill('SIGTERM');
    setTimeout(() => {
      if (child.exitCode === null) child.kill('SIGKILL');
    }, 2000);
  });
}

test('GET /health from shipped server.js', async (t) => {
  const { child, port } = await startServer();
  t.after(() => stopServer(child));
  const res = await request(port, 'GET', '/health');
  assert.equal(res.status, 200);
  assert.ok(res.json);
  assert.equal(res.json.ok, true);
  assert.equal(res.json.service, 'tangiers-checkout');
  assert.equal(res.json.paymentsConfigured, false);
});

test('GET /api/bots/catalog includes Ace / ace-grok-bot', async (t) => {
  const { child, port } = await startServer();
  t.after(() => stopServer(child));
  const res = await request(port, 'GET', '/api/bots/catalog');
  assert.equal(res.status, 200);
  assert.ok(res.json);
  assert.equal(res.json.ok, true);
  assert.ok(Array.isArray(res.json.bots));
  const ids = res.json.bots.map((b) => b && b.id);
  const names = res.json.bots.map((b) => b && b.name);
  assert.ok(ids.includes('ace-grok-bot'), 'catalog ids: ' + JSON.stringify(ids));
  assert.ok(
    names.some((n) => typeof n === 'string' && /Ace/i.test(n)),
    'catalog names: ' + JSON.stringify(names)
  );
});

test('POST /api/checkout/session without Stripe secret returns payment-link fallback', async (t) => {
  const { child, port } = await startServer();
  t.after(() => stopServer(child));
  const res = await request(port, 'POST', '/api/checkout/session', {
    botId: 'ace-grok-bot',
  });
  assert.equal(res.status, 200);
  assert.ok(res.json);
  assert.equal(res.json.ok, true);
  assert.equal(res.json.fallback, true);
  assert.equal(typeof res.json.fallbackUrl, 'string');
  assert.ok(res.json.fallbackUrl.length > 0, 'fallbackUrl must be a payment-link URL');
  assert.equal(typeof res.json.url, 'string');
  assert.ok(res.json.url.length > 0, 'url must be a payment-link URL');
});
