# Copyright 2025 t54 labs
# SPDX-License-Identifier: Apache-2.0
"""Drive the shipped facilitator runner: bind address + GET /health."""

from fastapi.testclient import TestClient

from run_facilitator_proxy import app, listen_address


def test_listen_address_prefers_platform_port(monkeypatch) -> None:
    monkeypatch.setenv("PORT", "8080")
    monkeypatch.setenv("PROXY_PORT", "8000")
    monkeypatch.setenv("PROXY_HOST", "0.0.0.0")
    monkeypatch.delenv("PROXY_RELOAD", raising=False)
    host, port, reload = listen_address()
    assert host == "0.0.0.0"
    assert port == 8080
    assert reload is False


def test_listen_address_uses_proxy_port_without_platform_port(monkeypatch) -> None:
    monkeypatch.delenv("PORT", raising=False)
    monkeypatch.setenv("PROXY_PORT", "9001")
    monkeypatch.delenv("PROXY_RELOAD", raising=False)
    _host, port, reload = listen_address()
    assert port == 9001
    assert reload is True


def test_listen_address_reload_override(monkeypatch) -> None:
    monkeypatch.setenv("PORT", "8080")
    monkeypatch.setenv("PROXY_RELOAD", "1")
    _host, port, reload = listen_address()
    assert port == 8080
    assert reload is True


def test_shipped_app_health_status_ok(test_env) -> None:
    client = TestClient(app)
    response = client.get("/health")
    assert response.status_code == 200
    body = response.json()
    assert body["status"] == "ok"
    assert "upstream_verify" in body
    assert "upstream_settle" in body
