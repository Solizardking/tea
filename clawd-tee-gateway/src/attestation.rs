//! TEE attestation: TDX quote collection + NVIDIA GPU attestation via NRAS.
//!
//! TDX quote collection:
//!   On a real TDX VM, Linux exposes `/dev/tdx_guest`. We issue an ioctl
//!   `TDX_CMD_GET_QUOTE` with a 64-byte report_data buffer containing:
//!     bytes  0–19: signing_address (20 bytes, zero-padded to 32)
//!     bytes 32–63: per-boot nonce (32 bytes)
//!   The kernel driver forwards this to the TDX module and returns a
//!   DCAP quote. The quote hash is SHA256 of those raw bytes.
//!
//!   When NOT running inside TDX (dev/CI), `TdxBackend::Stub` is used
//!   and returns a dummy quote so the rest of the flow can be tested.
//!
//! NVIDIA GPU attestation:
//!   We call NRAS with the GPU evidence payload and decode the JWT to
//!   extract the `x-nvidia-overall-att-result` claim.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::types::NrasClaims;

// ---------------------------------------------------------------------------
// TDX
// ---------------------------------------------------------------------------

pub enum TdxBackend {
    /// Real TDX VM: issue ioctl against /dev/tdx_guest.
    Real,
    /// Stub for dev/CI: returns a deterministic dummy quote.
    Stub,
}

impl TdxBackend {
    pub fn detect() -> Self {
        if std::path::Path::new("/dev/tdx_guest").exists() {
            TdxBackend::Real
        } else {
            warn!("/dev/tdx_guest not found — using stub TDX backend (not in a TDX VM)");
            TdxBackend::Stub
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TdxBackend::Real => "real",
            TdxBackend::Stub => "stub",
        }
    }

    /// Collect a TDX quote binding `report_data` (64 bytes) into the quote.
    /// Returns raw quote bytes.
    pub fn get_quote(&self, report_data: &[u8; 64]) -> Result<Vec<u8>> {
        match self {
            TdxBackend::Real => get_tdx_quote_real(report_data),
            TdxBackend::Stub => Ok(get_tdx_quote_stub(report_data)),
        }
    }
}

/// Legacy CLAWD report_data: addr(0–19 zero-padded to 32) || nonce(32–63).
/// ACI quotes use [`crate::aci::report_data_slot`] instead.
#[allow(dead_code)]
pub fn build_report_data(signing_address: &[u8; 20], nonce: &[u8; 32]) -> [u8; 64] {
    let mut rd = [0u8; 64];
    rd[0..20].copy_from_slice(signing_address);
    // bytes 20–31 stay zero (padding)
    rd[32..64].copy_from_slice(nonce);
    rd
}

/// Issue TDX_CMD_GET_QUOTE ioctl on /dev/tdx_guest.
/// The ioctl number and struct layout follow linux/tdx-guest.h (kernel 6.7+).
fn get_tdx_quote_real(report_data: &[u8; 64]) -> Result<Vec<u8>> {
    use std::os::unix::io::AsRawFd;

    // TDX_CMD_GET_QUOTE ioctl: _IOWR(0x54, 0x04, struct tdx_quote_hdr)
    // struct tdx_quote_hdr {
    //   __u64 version;   // = 1
    //   __u64 status;
    //   __u32 in_len;    // = 64
    //   __u32 out_len;   // filled by kernel
    //   __u8  data[4 + in_len + out_quote_len];
    // }
    const TDX_CMD_GET_QUOTE: u64 = 0xc008_5404; // _IOWR(0x54, 0x04, u64)

    #[repr(C)]
    struct TdxQuoteHdr {
        version: u64,
        status: u64,
        in_len: u32,
        out_len: u32,
        data: [u8; 8192], // large enough for typical quote
    }

    let file = std::fs::File::open("/dev/tdx_guest").context("opening /dev/tdx_guest")?;

    let mut hdr = TdxQuoteHdr {
        version: 1,
        status: 0,
        in_len: 64,
        out_len: 0,
        data: [0u8; 8192],
    };
    hdr.data[4..68].copy_from_slice(report_data);

    let ret = unsafe {
        libc_ioctl(
            file.as_raw_fd(),
            TDX_CMD_GET_QUOTE,
            &mut hdr as *mut _ as u64,
        )
    };

    if ret != 0 {
        anyhow::bail!("TDX_CMD_GET_QUOTE ioctl failed: errno {ret}");
    }
    if hdr.status != 0 {
        anyhow::bail!("TDX quote status: {}", hdr.status);
    }

    let quote_len = hdr.out_len as usize;
    let quote_start = 4 + 64; // skip header bytes + report_data
    Ok(hdr.data[quote_start..quote_start + quote_len].to_vec())
}

// Minimal libc ioctl binding — avoids pulling in the full libc crate.
extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}
unsafe fn libc_ioctl(fd: i32, request: u64, arg: u64) -> i32 {
    ioctl(fd, request, arg)
}

fn get_tdx_quote_stub(report_data: &[u8; 64]) -> Vec<u8> {
    // Deterministic stub: SHA256(report_data) padded to 256 bytes.
    let hash = Sha256::digest(report_data);
    let mut quote = vec![0xCAu8; 256];
    quote[0..32].copy_from_slice(&hash);
    quote
}

// ---------------------------------------------------------------------------
// NVIDIA GPU attestation via NRAS
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NrasEvidence {
    pub nonce: String,
    /// Raw GPU evidence JSON collected via NVML.
    /// In production: call nvmlDeviceGetConfComputeGpuAttestationReport()
    /// and serialize the result. Here we accept a pre-collected payload.
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// Submit GPU evidence to NRAS and return attestation claims.
/// Returns `None` if no NVIDIA API key is configured.
pub async fn attest_gpu(
    nras_url: &str,
    nvidia_api_key: Option<&str>,
    evidence: NrasEvidence,
) -> Result<Option<NrasClaims>> {
    let Some(api_key) = nvidia_api_key else {
        debug!("no NVIDIA_API_KEY — skipping GPU attestation");
        return Ok(None);
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(nras_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&evidence)
        .send()
        .await
        .context("NRAS request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("NRAS returned HTTP {status}: {body}");
    }

    // NRAS returns an array: [[claim_type, jwt_token], ...]
    let tokens: Vec<Vec<String>> = resp.json().await.context("parsing NRAS response")?;
    let jwt = tokens
        .into_iter()
        .find(|pair| pair.len() == 2)
        .and_then(|pair| pair.into_iter().nth(1))
        .context("no JWT in NRAS response")?;

    let claims = decode_nras_jwt(&jwt)?;
    Ok(Some(claims))
}

fn decode_nras_jwt(jwt: &str) -> Result<NrasClaims> {
    let parts: Vec<&str> = jwt.splitn(3, '.').collect();
    if parts.len() < 2 {
        anyhow::bail!("malformed JWT");
    }
    let payload_b64 = parts[1];
    let padding = (4 - payload_b64.len() % 4) % 4;
    let padded = format!("{payload_b64}{}", "=".repeat(padding));
    let decoded = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&padded))
        .context("base64-decoding JWT payload")?;

    let claims: serde_json::Value =
        serde_json::from_slice(&decoded).context("parsing JWT payload JSON")?;

    Ok(NrasClaims {
        overall_result: claims["x-nvidia-overall-att-result"]
            .as_bool()
            .or_else(|| {
                claims["x-nvidia-overall-att-result"]
                    .as_str()
                    .map(|s| s == "true")
            })
            .unwrap_or(false),
        gpu_uuid: claims["x-nvidia-gpu-uuid"].as_str().map(str::to_string),
        driver_version: claims["x-nvidia-driver-version"]
            .as_str()
            .map(str::to_string),
        cc_mode: claims["x-nvidia-confidential-compute-mode"]
            .as_str()
            .map(str::to_string),
    })
}

// ---------------------------------------------------------------------------
// Collect GPU evidence via NVML
// ---------------------------------------------------------------------------

/// Collect GPU attestation evidence from the local GPU.
///
/// On a real system this calls nvmlDeviceGetConfComputeGpuAttestationReport().
/// For now it returns a stub payload; replace with the NVIDIA Rust SDK when
/// it is available as a crate (https://docs.nvidia.com/attestation/nv-attestation-sdk-cpp/latest/sdk-rust/).
pub fn collect_gpu_evidence(nonce: &[u8; 32]) -> NrasEvidence {
    let nonce_hex = hex::encode(nonce);

    // Stub — replace with actual NVML call:
    //   nvmlInit()
    //   nvmlDeviceGetHandleByIndex(0, &device)
    //   nvmlDeviceGetConfComputeGpuAttestationReport(device, &report)
    //   serialize report to JSON
    NrasEvidence {
        nonce: nonce_hex,
        payload: serde_json::json!({
            "stub": true,
            "message": "replace collect_gpu_evidence() with NVML ConfComputeGpuAttestationReport"
        }),
    }
}
