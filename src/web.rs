//! HTTP server: static UI + small JSON API.
//!
//! GET  /            -> web UI
//! GET  /api/status  -> ControllerState as JSON
//! POST /api/settings  {"target_temp_c": 42.5, "enabled": true}  (both optional)

use anyhow::Result;
use embedded_svc::http::Headers;
use esp_idf_svc::http::Method;
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::io::{Read, Write};
use serde::Deserialize;

use crate::config;
use crate::state::SharedState;

static INDEX_HTML: &str = include_str!("web/index.html");
const MAX_BODY: usize = 256;

#[derive(Deserialize)]
struct Settings {
    // f64 on purpose: serde_json's f32 path trips an Xtensa LLVM codegen bug ("Cannot select PCREL_WRAPPER").
    target_temp_c: Option<f64>,
    enabled: Option<bool>,
}

pub fn start(state: SharedState) -> Result<EspHttpServer<'static>> {
    let mut server = EspHttpServer::new(&Configuration { stack_size: 10240, ..Default::default() })?;

    server.fn_handler("/", Method::Get, |req| {
        req.into_response(200, None, &[("Content-Type", "text/html; charset=utf-8")])?
            .write_all(INDEX_HTML.as_bytes())
    })?;

    let st = state.clone();
    server.fn_handler::<anyhow::Error, _>("/api/status", Method::Get, move |req| {
        let body = serde_json::to_vec(&*st.lock().unwrap())?;
        req.into_response(200, None, &[("Content-Type", "application/json"), ("Cache-Control", "no-store")])?
            .write_all(&body)?;
        Ok(())
    })?;

    let st = state;
    server.fn_handler::<anyhow::Error, _>("/api/settings", Method::Post, move |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;
        if len == 0 || len > MAX_BODY {
            req.into_status_response(413)?.write_all(b"bad body size")?;
            return Ok(());
        }
        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;

        let settings: Settings = match serde_json::from_slice(&buf) {
            Ok(s) => s,
            Err(e) => {
                req.into_status_response(400)?.write_all(format!("invalid JSON: {e}").as_bytes())?;
                return Ok(());
            }
        };

        let target = settings.target_temp_c.map(|t| t as f32);
        if let Some(t) = target {
            if !t.is_finite() || !(config::MIN_TARGET_C..=config::MAX_TARGET_C).contains(&t) {
                let msg = format!("target must be {}..{} °C", config::MIN_TARGET_C, config::MAX_TARGET_C);
                req.into_status_response(422)?.write_all(msg.as_bytes())?;
                return Ok(());
            }
        }

        let body = {
            let mut s = st.lock().unwrap();
            if let Some(t) = target {
                s.target_temp_c = (t * 10.0).round() / 10.0;
            }
            if let Some(e) = settings.enabled {
                s.enabled = e;
            }
            serde_json::to_vec(&*s)?
        };
        req.into_response(200, None, &[("Content-Type", "application/json")])?.write_all(&body)?;
        Ok(())
    })?;

    Ok(server)
}
