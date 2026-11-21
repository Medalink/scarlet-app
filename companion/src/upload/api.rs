//! A small blocking client for the upload API: signing as docs/API.md says, retries, and a
//! record of the bytes of every request.

use super::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use scarlet_part::pseudo::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const API: &str = "/api/v1";
const USER_AGENT: &str = "scarlet-upload";
/// Tries for a network error or a 5xx, and for a 429.
const TRIES: usize = 5;
const RATE_TRIES: usize = 3;
/// The longest a 429 is waited out in one go.
const MAX_WAIT: u64 = 120;

/// One request as it went over the wire, without anything that names a character.
#[derive(Clone, Debug)]
pub struct Hit {
    pub method: &'static str,
    pub path: String,
    pub status: u16,
    /// Request body bytes.
    pub sent_body: u64,
    /// Request line and headers, counted from the fields set here plus `Host` and `User-Agent`.
    pub sent_head: u64,
    pub recv_body: u64,
    /// Status line and headers.
    pub recv_head: u64,
    pub ms: u64,
}

impl Hit {
    pub fn sent(&self) -> u64 {
        self.sent_body + self.sent_head
    }
    pub fn received(&self) -> u64 {
        self.recv_body + self.recv_head
    }
}

pub struct Reply {
    pub status: u16,
    pub json: Value,
}

pub struct Client {
    agent: ureq::Agent,
    site: String,
    key: Option<SigningKey>,
    install: Option<String>,
    /// The last time signed for each message, so the same request is never signed twice for one second.
    signed_at: HashMap<String, u64>,
    pub hits: Vec<Hit>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Client {
    /// `site` is the website's address; the API is under `/api/v1` there.
    pub fn new(site: &str, seed: Option<&[u8; 32]>) -> Client {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(180)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Client {
            agent,
            site: super::config::site(site),
            key: seed.map(SigningKey::from_bytes),
            install: None,
            signed_at: HashMap::new(),
            hits: Vec::new(),
        }
    }

    pub fn site(&self) -> &str {
        &self.site
    }

    pub fn set_install(&mut self, id: &str) {
        self.install = Some(id.to_string());
    }

    pub fn public_key(&self) -> Option<String> {
        self.key
            .as_ref()
            .map(|k| STANDARD.encode(k.verifying_key().to_bytes()))
    }

    /// Request without a body.
    pub fn call(&mut self, method: &'static str, path: &str) -> Result<Reply> {
        self.send(method, path, None, true)
    }

    /// Request with a body of the given content type.
    pub fn call_body(
        &mut self,
        method: &'static str,
        path: &str,
        content_type: &str,
        body: &[u8],
    ) -> Result<Reply> {
        self.send(method, path, Some((content_type, body)), true)
    }

    /// `POST /installs`, the one request that is not signed.
    pub fn register(&mut self, name: &str) -> Result<Reply> {
        let public_key = self
            .public_key()
            .ok_or_else(|| Error::local("no install key"))?;
        let body = serde_json::json!({ "public_key": public_key, "name": name }).to_string();
        self.send(
            "POST",
            "/installs",
            Some(("application/json", body.as_bytes())),
            false,
        )
    }

    fn send(
        &mut self,
        method: &'static str,
        path: &str,
        body: Option<(&str, &[u8])>,
        signed: bool,
    ) -> Result<Reply> {
        let full = format!("{API}{path}");
        let url = format!("{}{full}", self.site);
        let bytes: &[u8] = body.map_or(&[][..], |b| b.1);
        let body_hash = hex(&Sha256::digest(bytes));
        let (mut net_tries, mut rate_tries) = (0, 0);
        loop {
            let mut headers: Vec<(&'static str, String)> = Vec::new();
            if signed {
                let key = self
                    .key
                    .as_ref()
                    .ok_or_else(|| Error::local("no install key"))?;
                let install = self
                    .install
                    .clone()
                    .ok_or_else(|| Error::local("this install is not registered"))?;
                let id = format!("{method}\n{full}\n{body_hash}");
                let last = self.signed_at.get(&id).copied();
                let time = last.map_or(now(), |l| (l + 1).max(now()));
                self.signed_at.insert(id, time);
                let message = format!("{method}\n{full}\n{time}\n{body_hash}");
                let sig = STANDARD.encode(key.sign(message.as_bytes()).to_bytes());
                headers.push(("Scarlet-Install", install));
                headers.push(("Scarlet-Time", time.to_string()));
                headers.push(("Scarlet-Signature", sig));
            }
            let started = Instant::now();
            let outcome = self.once(method, &url, &headers, body);
            let ms = started.elapsed().as_millis() as u64;
            match outcome {
                Err(e) => {
                    net_tries += 1;
                    if net_tries >= TRIES {
                        return Err(Error::Net(e));
                    }
                    std::thread::sleep(Duration::from_secs(1 << (net_tries - 1)));
                }
                Ok((status, head, text, retry_after)) => {
                    let json: Value = serde_json::from_slice(&text).unwrap_or(Value::Null);
                    self.hits.push(Hit {
                        method,
                        path: full.clone(),
                        status,
                        sent_body: bytes.len() as u64,
                        sent_head: request_head(method, &full, &self.site, &headers, body),
                        recv_body: text.len() as u64,
                        recv_head: head,
                        ms,
                    });
                    if status < 300 {
                        return Ok(Reply { status, json });
                    }
                    if status >= 500 {
                        net_tries += 1;
                        if net_tries >= TRIES {
                            return Err(api_error(status, &json));
                        }
                        std::thread::sleep(Duration::from_secs(1 << (net_tries - 1)));
                    } else if status == 429 {
                        rate_tries += 1;
                        if rate_tries >= RATE_TRIES {
                            return Err(api_error(status, &json));
                        }
                        let wait = retry_after
                            .or_else(|| json.pointer("/error/retry_after").and_then(Value::as_u64))
                            .unwrap_or(5)
                            .clamp(1, MAX_WAIT);
                        std::thread::sleep(Duration::from_secs(wait));
                    } else {
                        return Err(api_error(status, &json));
                    }
                }
            }
        }
    }

    /// One try: status, response head bytes, body, `Retry-After`.
    fn once(
        &self,
        method: &str,
        url: &str,
        headers: &[(&'static str, String)],
        body: Option<(&str, &[u8])>,
    ) -> std::result::Result<(u16, u64, Vec<u8>, Option<u64>), String> {
        let mut resp = match (method, body) {
            ("GET", _) => with(self.agent.get(url), headers).call(),
            ("DELETE", _) => with(self.agent.delete(url), headers).call(),
            ("PUT", None) => with(self.agent.put(url), headers).send_empty(),
            ("POST", None) => with(self.agent.post(url), headers).send_empty(),
            ("POST", Some((ct, b))) => with(self.agent.post(url), headers).content_type(ct).send(b),
            ("PUT", Some((ct, b))) => with(self.agent.put(url), headers).content_type(ct).send(b),
            _ => return Err(format!("unsupported method {method}")),
        }
        .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let mut head = format!("HTTP/1.1 {status} {}\r\n", resp.status().as_str()).len() as u64;
        let mut retry_after = None;
        for (name, value) in resp.headers() {
            head += (name.as_str().len() + value.len() + 4) as u64;
            if name.as_str().eq_ignore_ascii_case("retry-after") {
                retry_after = value.to_str().ok().and_then(|v| v.trim().parse().ok());
            }
        }
        head += 2;
        let text = resp
            .body_mut()
            .with_config()
            .limit(16 << 20)
            .read_to_vec()
            .map_err(|e| e.to_string())?;
        Ok((status, head, text, retry_after))
    }
}

fn with<B>(
    mut req: ureq::RequestBuilder<B>,
    headers: &[(&'static str, String)],
) -> ureq::RequestBuilder<B> {
    for (k, v) in headers {
        req = req.header(*k, v.as_str());
    }
    req
}

fn request_head(
    method: &str,
    path: &str,
    site: &str,
    headers: &[(&'static str, String)],
    body: Option<(&str, &[u8])>,
) -> u64 {
    let host = site.split("://").nth(1).unwrap_or(site);
    let mut n = format!("{method} {path} HTTP/1.1\r\n").len() as u64;
    n += format!("Host: {host}\r\nUser-Agent: {USER_AGENT}\r\n").len() as u64;
    for (k, v) in headers {
        n += (k.len() + v.len() + 4) as u64;
    }
    if let Some((ct, b)) = body {
        n += format!("Content-Type: {ct}\r\nContent-Length: {}\r\n", b.len()).len() as u64;
    }
    n + 2
}

fn api_error(status: u16, json: &Value) -> Error {
    let e = json.get("error");
    let text = |k: &str| {
        e.and_then(|e| e.get(k))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    Error::Api {
        status,
        code: text("code"),
        message: text("message"),
        extra: e.cloned().unwrap_or(Value::Null),
    }
}
