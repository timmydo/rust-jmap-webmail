use base64::Engine;
use serde_json::json;

use super::types::*;

pub struct JmapClient {
    username: String,
    password: String,
    api_url: String,
    account_id: String,
}

#[derive(Debug)]
pub enum JmapError {
    Http(String),
    Parse(String),
    Api(String),
}

impl std::fmt::Display for JmapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JmapError::Http(e) => write!(f, "HTTP error: {}", e),
            JmapError::Parse(e) => write!(f, "Parse error: {}", e),
            JmapError::Api(e) => write!(f, "API error: {}", e),
        }
    }
}

impl JmapClient {
    fn auth_header(username: &str, password: &str) -> String {
        let credentials = format!("{}:{}", username, password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        format!("Basic {}", encoded)
    }

    /// Resolve a URL, following redirects manually while preserving the auth header.
    /// Returns the final URL and the response body.
    fn fetch_with_auth_following_redirects(
        url: &str,
        auth: &str,
        max_redirects: u32,
    ) -> Result<(String, String), JmapError> {
        let agent = ureq::AgentBuilder::new()
            .redirects(0) // Don't auto-follow, we'll handle manually
            .build();

        let mut current_url = url.to_string();

        for i in 0..max_redirects {
            eprintln!("[JMAP] Request {} to: {}", i + 1, current_url);

            let response = agent
                .get(&current_url)
                .set("Authorization", auth)
                .call();

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    eprintln!("[JMAP] Got {} response", status);

                    // Check if this is a redirect (ureq may return 3xx as Ok)
                    if (300..400).contains(&status) {
                        if let Some(location) = resp.header("location") {
                            eprintln!("[JMAP] Following redirect {} -> {}", status, location);
                            current_url = Self::resolve_redirect(&current_url, location);
                            continue;
                        } else {
                            return Err(JmapError::Http(format!(
                                "Redirect {} without Location header",
                                status
                            )));
                        }
                    }

                    // Success - return the response body
                    let body = resp
                        .into_string()
                        .map_err(|e| JmapError::Parse(format!("Failed to read response: {}", e)))?;

                    if body.is_empty() {
                        return Err(JmapError::Http(format!(
                            "Server returned empty response (status {})",
                            status
                        )));
                    }

                    return Ok((current_url, body));
                }
                Err(ureq::Error::Status(code, resp)) if (300..400).contains(&code) => {
                    // Redirect returned as error - follow it with auth header preserved
                    if let Some(location) = resp.header("location") {
                        eprintln!("[JMAP] Following redirect {} -> {}", code, location);
                        current_url = Self::resolve_redirect(&current_url, location);
                    } else {
                        return Err(JmapError::Http(format!(
                            "Redirect {} without Location header",
                            code
                        )));
                    }
                }
                Err(ureq::Error::Status(code, resp)) => {
                    // HTTP error (4xx, 5xx)
                    let body = resp.into_string().unwrap_or_default();
                    eprintln!("[JMAP] HTTP error {}: {}", code, body);

                    if code == 401 {
                        return Err(JmapError::Http(
                            "Authentication failed (401 Unauthorized)".to_string(),
                        ));
                    }

                    return Err(JmapError::Http(format!(
                        "HTTP {} error: {}",
                        code,
                        if body.is_empty() {
                            "(empty response)".to_string()
                        } else {
                            truncate_str(&body, 200).to_string()
                        }
                    )));
                }
                Err(e) => {
                    eprintln!("[JMAP] Connection error: {}", e);
                    return Err(JmapError::Http(e.to_string()));
                }
            }
        }

        Err(JmapError::Http("Too many redirects".to_string()))
    }

    /// Resolve a redirect location (which may be relative) against a base URL
    fn resolve_redirect(base_url: &str, location: &str) -> String {
        if location.starts_with("http://") || location.starts_with("https://") {
            // Absolute URL
            location.to_string()
        } else if location.starts_with('/') {
            // Absolute path - need to extract scheme + host from base
            if let Some(idx) = base_url.find("://") {
                let after_scheme = &base_url[idx + 3..];
                if let Some(path_start) = after_scheme.find('/') {
                    let host_part = &base_url[..idx + 3 + path_start];
                    format!("{}{}", host_part, location)
                } else {
                    format!("{}{}", base_url, location)
                }
            } else {
                location.to_string()
            }
        } else {
            // Relative path
            if let Some(last_slash) = base_url.rfind('/') {
                format!("{}/{}", &base_url[..last_slash], location)
            } else {
                location.to_string()
            }
        }
    }

    pub fn discover(
        well_known_url: &str,
        username: &str,
        password: &str,
    ) -> Result<(JmapSession, Self), JmapError> {
        let auth = Self::auth_header(username, password);

        // Fetch the session, following redirects while preserving auth header
        let (_final_url, response_text) =
            Self::fetch_with_auth_following_redirects(well_known_url, &auth, 5)?;

        let session: JmapSession = serde_json::from_str(&response_text)
            .map_err(|e| JmapError::Parse(format!("Failed to parse session: {}. Response was: {}", e, truncate_str(&response_text, 500))))?;

        let account_id = session
            .mail_account_id()
            .ok_or_else(|| {
                let primary_caps: Vec<_> = session.primary_accounts.keys().collect();
                let account_ids: Vec<_> = session.accounts.keys().collect();
                JmapError::Api(format!(
                    "No mail account found. primaryAccounts: {:?}, accounts: {:?}. Full response: {}",
                    primary_caps,
                    account_ids,
                    truncate_str(&response_text, 500)
                ))
            })?
            .to_string();

        let client = JmapClient {
            username: username.to_string(),
            password: password.to_string(),
            api_url: session.api_url.clone(),
            account_id,
        };

        Ok((session, client))
    }

    pub fn from_session(
        username: String,
        password: String,
        api_url: String,
        account_id: String,
    ) -> Self {
        JmapClient {
            username,
            password,
            api_url,
            account_id,
        }
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    fn call(&self, request: JmapRequest) -> Result<JmapResponse, JmapError> {
        let auth = Self::auth_header(&self.username, &self.password);

        let response = ureq::post(&self.api_url)
            .set("Authorization", &auth)
            .set("Content-Type", "application/json")
            .send_json(&request)
            .map_err(|e| JmapError::Http(e.to_string()))?;

        response
            .into_json()
            .map_err(|e| JmapError::Parse(e.to_string()))
    }

    pub fn get_mailboxes(&self) -> Result<Vec<Mailbox>, JmapError> {
        let request = JmapRequest {
            using: vec!["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            method_calls: vec![MethodCall(
                "Mailbox/get",
                json!({
                    "accountId": self.account_id,
                    "ids": null
                }),
                "0".to_string(),
            )],
        };

        let response = self.call(request)?;

        if let Some(method_response) = response.method_responses.first() {
            if method_response.0 == "Mailbox/get" {
                let mailbox_response: MailboxGetResponse =
                    serde_json::from_value(method_response.1.clone())
                        .map_err(|e| JmapError::Parse(e.to_string()))?;
                return Ok(mailbox_response.list);
            }
        }

        Err(JmapError::Api("Unexpected response".to_string()))
    }

    pub fn query_emails(
        &self,
        mailbox_id: &str,
        limit: u32,
    ) -> Result<Vec<String>, JmapError> {
        let request = JmapRequest {
            using: vec!["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            method_calls: vec![MethodCall(
                "Email/query",
                json!({
                    "accountId": self.account_id,
                    "filter": { "inMailbox": mailbox_id },
                    "sort": [{ "property": "receivedAt", "isAscending": false }],
                    "limit": limit
                }),
                "0".to_string(),
            )],
        };

        let response = self.call(request)?;

        if let Some(method_response) = response.method_responses.first() {
            if method_response.0 == "Email/query" {
                let query_response: EmailQueryResponse =
                    serde_json::from_value(method_response.1.clone())
                        .map_err(|e| JmapError::Parse(e.to_string()))?;
                return Ok(query_response.ids);
            }
        }

        Err(JmapError::Api("Unexpected response".to_string()))
    }

    pub fn get_emails(&self, ids: &[String]) -> Result<Vec<Email>, JmapError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let request = JmapRequest {
            using: vec!["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            method_calls: vec![MethodCall(
                "Email/get",
                json!({
                    "accountId": self.account_id,
                    "ids": ids,
                    "properties": [
                        "id", "from", "to", "cc", "subject",
                        "receivedAt", "preview", "textBody", "bodyValues", "keywords"
                    ],
                    "fetchTextBodyValues": true
                }),
                "0".to_string(),
            )],
        };

        let response = self.call(request)?;

        if let Some(method_response) = response.method_responses.first() {
            if method_response.0 == "Email/get" {
                let email_response: EmailGetResponse =
                    serde_json::from_value(method_response.1.clone())
                        .map_err(|e| JmapError::Parse(e.to_string()))?;
                return Ok(email_response.list);
            }
        }

        Err(JmapError::Api("Unexpected response".to_string()))
    }

    pub fn get_email(&self, id: &str) -> Result<Option<Email>, JmapError> {
        let emails = self.get_emails(&[id.to_string()])?;
        Ok(emails.into_iter().next())
    }
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}
