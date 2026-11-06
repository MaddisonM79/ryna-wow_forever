//! The one HTTP call the agent makes (a POST), behind a trait so sign-in and pushing can be tested
//! without a network.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Request {
    pub url: String,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub bearer: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }

    /// The start of the body, for error messages.
    pub fn excerpt(&self) -> String {
        let text = String::from_utf8_lossy(&self.body);
        let mut s: String = text.chars().take(300).collect();
        if text.chars().count() > 300 {
            s.push('…');
        }
        s
    }
}

/// POST and return whatever came back. `Err` means no answer (connection, TLS, timeout).
pub trait Transport {
    fn post(&self, req: &Request) -> Result<Response, String>;
}

/// The real one: reqwest's blocking client with rustls.
pub struct HttpTransport {
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(crate::user_agent())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(HttpTransport { client })
    }
}

impl Transport for HttpTransport {
    fn post(&self, req: &Request) -> Result<Response, String> {
        let mut r = self
            .client
            .post(&req.url)
            .header("Content-Type", req.content_type)
            .header("Accept", "application/json")
            .timeout(req.timeout)
            .body(req.body.clone());
        if let Some(token) = &req.bearer {
            r = r.bearer_auth(token);
        }
        let res = r.send().map_err(|e| describe(&e))?;
        let status = res.status().as_u16();
        let content_type = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = res.bytes().map_err(|e| describe(&e))?.to_vec();
        Ok(Response {
            status,
            content_type,
            body,
        })
    }
}

/// reqwest's error with its causes, which is where the useful part (DNS, refused, TLS) is.
fn describe(e: &reqwest::Error) -> String {
    let mut s = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        s.push_str(": ");
        s.push_str(&cause.to_string());
        source = cause.source();
    }
    s
}

/// `application/x-www-form-urlencoded`.
pub fn form_encode(fields: &[(&str, &str)]) -> Vec<u8> {
    fn enc(s: &str, out: &mut String) {
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    out.push(b as char)
                }
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
    }
    let mut out = String::new();
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        enc(k, &mut out);
        out.push('=');
        enc(v, &mut out);
    }
    out.into_bytes()
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Answers from a script, in order, and remembers every request.
    #[derive(Default)]
    pub struct ScriptedTransport {
        pub answers: RefCell<VecDeque<Result<Response, String>>>,
        pub requests: RefCell<Vec<Request>>,
    }

    impl ScriptedTransport {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn json(&self, status: u16, body: serde_json::Value) -> &Self {
            self.answers.borrow_mut().push_back(Ok(Response {
                status,
                content_type: "application/json".into(),
                body: body.to_string().into_bytes(),
            }));
            self
        }
        pub fn text(&self, status: u16, body: &str) -> &Self {
            self.answers.borrow_mut().push_back(Ok(Response {
                status,
                content_type: "text/html".into(),
                body: body.as_bytes().to_vec(),
            }));
            self
        }
        pub fn fail(&self, why: &str) -> &Self {
            self.answers.borrow_mut().push_back(Err(why.to_string()));
            self
        }
        pub fn urls(&self) -> Vec<String> {
            self.requests
                .borrow()
                .iter()
                .map(|r| r.url.clone())
                .collect()
        }
        pub fn form(&self, i: usize) -> String {
            String::from_utf8(self.requests.borrow()[i].body.clone()).unwrap()
        }
        pub fn remaining(&self) -> usize {
            self.answers.borrow().len()
        }
    }

    impl Transport for ScriptedTransport {
        fn post(&self, req: &Request) -> Result<Response, String> {
            self.requests.borrow_mut().push(req.clone());
            self.answers
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted answer for {}", req.url))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_encoding() {
        let f = form_encode(&[
            ("scope", "openid wow:ingest"),
            ("resource", "https://api.ryna.io/wow/ingest"),
            ("x", "a&b=c/é"),
        ]);
        assert_eq!(
            String::from_utf8(f).unwrap(),
            "scope=openid+wow%3Aingest&resource=https%3A%2F%2Fapi.ryna.io%2Fwow%2Fingest&x=a%26b%3Dc%2F%C3%A9"
        );
    }
}
