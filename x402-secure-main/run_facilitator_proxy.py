#!/usr/bin/env python3
# Copyright 2025 t54 labs
# SPDX-License-Identifier: Apache-2.0
"""Standalone runner for the x402 Facilitator Proxy.

Environment Variables:
  - PORT (Railway/platform bind port; preferred when set)
  - PROXY_PORT (default: 8000; used when PORT is unset)
  - PROXY_HOST (default: 0.0.0.0)
  - PROXY_RELOAD (1=uvicorn reloader; default 1 locally, 0 when PORT is set)
  - PROXY_UPSTREAM_VERIFY_URL (default: http://localhost:8001/verify)
  - PROXY_UPSTREAM_SETTLE_URL (default: http://localhost:8001/settle)
  - PROXY_TIMEOUT_S (default: 15)
"""

import logging
import os
from datetime import datetime, timezone

from dotenv import load_dotenv  # type: ignore
from fastapi import Depends, FastAPI
from x402_proxy import ProxyRuntimeConfig, get_proxy_cfg, internal_router, risk_router, router

logging.basicConfig(
    level=os.getenv("LOG_LEVEL", "INFO").upper(),
    format="%(asctime)s %(levelname)s %(name)s %(message)s",
)
logger = logging.getLogger("facilitator_proxy")


def build_app() -> FastAPI:
    """Build and configure the FastAPI application."""
    # Load .env file to make environment variables available
    load_dotenv()

    app = FastAPI(
        title="x402 Facilitator Proxy",
        description="AP2-enforcing proxy for x402 verify/settle",
        version="0.1.0",
    )

    # Wire runtime config via dependency
    def cfg_factory() -> ProxyRuntimeConfig:
        return ProxyRuntimeConfig()  # reads env defaults

    app.dependency_overrides[get_proxy_cfg] = cfg_factory  # type: ignore[arg-type]

    # Health
    @app.get("/health")
    async def health(cfg: ProxyRuntimeConfig = Depends(get_proxy_cfg)) -> dict:
        return {
            "status": "ok",
            "time": datetime.now(timezone.utc).isoformat(),
            "upstream_verify": cfg.upstream_verify_url,
            "upstream_settle": cfg.upstream_settle_url,
        }

    # Mount proxy router (/x402/*)
    app.include_router(router)
    # Mount public Risk API (/risk/*) at the unified gateway
    app.include_router(risk_router)
    # Mount hosted-facilitator internal API (/internal/x402-secure/facilitator/*)
    app.include_router(internal_router)

    logger.info("Facilitator Proxy app initialized")
    return app


app = build_app()


def listen_address() -> tuple[str, int, bool]:
    """Host/port/reload for uvicorn.

    Railway and other PaaS inject PORT. Prefer it over PROXY_PORT so the
    public proxy actually binds the platform port. The reloader is for local
    development; disable it whenever PORT is set unless PROXY_RELOAD=1.
    """
    host = os.getenv("PROXY_HOST") or os.getenv("HOST") or "0.0.0.0"
    port = int(os.getenv("PORT") or os.getenv("PROXY_PORT") or "8000")
    default_reload = "0" if os.getenv("PORT") else "1"
    reload = os.getenv("PROXY_RELOAD", default_reload) == "1"
    return host, port, reload


if __name__ == "__main__":
    import uvicorn

    host, port, reload = listen_address()
    uvicorn.run(
        "run_facilitator_proxy:app",
        host=host,
        port=port,
        reload=reload,
        log_level="info",
    )
