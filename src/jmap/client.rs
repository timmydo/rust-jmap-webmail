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

    fn resolve_session_url(well_known_url: &str) -> Result<String, JmapError> {
        // Follow redirects manually to get the final session URL
        // We do this without auth first, then authenticate to the final URL
        let agent = ureq::AgentBuilder::new()
            .redirects(0) // Don't auto-follow
            .build();

        let response = agent
            .get(well_known_url)
            .call();

        match response {
            Ok(_) => Ok(well_known_url.to_string()),
            Err(ureq::Error::Status(code, resp)) if (300..400).contains(&code) => {
                if let Some(location) = resp.header("location") {
                    // Handle relative URLs
                    if location.starts_with('/') {
                        // Extract base URL from well_known_url
                        if let Some(base) = well_known_url.find("/.well-known") {
                            Ok(format!("{}{}", &well_known_url[..base], location))
                        } else {
                            Ok(location.to_string())
                        }
                    } else {
                        Ok(location.to_string())
                    }
                } else {
                    Ok(well_known_url.to_string())
                }
            }
            Err(e) => Err(JmapError::Http(e.to_string())),
        }
    }

    pub fn discover(
        well_known_url: &str,
        username: &str,
        password: &str,
    ) -> Result<(JmapSession, Self), JmapError> {
        let auth = Self::auth_header(username, password);

        // First, resolve the well-known URL (may redirect)
        let session_url = Self::resolve_session_url(well_known_url)?;

        let response = ureq::get(&session_url)
            .set("Authorization", &auth)
            .call()
            .map_err(|e| JmapError::Http(e.to_string()))?;

        let response_text = response
            .into_string()
            .map_err(|e| JmapError::Parse(format!("Failed to read response: {}", e)))?;

        let session: JmapSession = serde_json::from_str(&response_text)
            .map_err(|e| JmapError::Parse(format!("Failed to parse session: {}. Response was: {}", e, truncate_str(&response_text, 500))))?;

        let account_id = session
            .mail_account_id()
            .ok_or_else(|| {
                let caps: Vec<_> = session.primary_accounts.keys().collect();
                JmapError::Api(format!(
                    "No mail account found. Available capabilities: {:?}. Full response: {}",
                    caps,
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
