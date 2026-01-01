use std::sync::Arc;
use tiny_http::{Header, Request, Response};
use uuid::Uuid;

use crate::config::Config;
use crate::jmap::JmapClient;
use crate::session::{
    clear_session_cookie, make_session_cookie, parse_session_cookie, Session, SessionStore,
};
use crate::templates;

pub struct AppState {
    pub config: Config,
    pub sessions: SessionStore,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        AppState {
            config,
            sessions: SessionStore::new(),
        }
    }
}

type BoxResponse = Response<std::io::Cursor<Vec<u8>>>;

pub fn handle_request(state: &Arc<AppState>, request: Request) {
    let path = request.url().to_string();
    let method = request.method().to_string();

    // Extract session if present
    let session_id = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "cookie")
        .and_then(|h| parse_session_cookie(h.value.as_str()));

    let response = route(state, &method, &path, session_id, request);

    // Ignore send errors (client may have disconnected)
    let _ = response;
}

fn route(
    state: &Arc<AppState>,
    method: &str,
    path: &str,
    session_id: Option<Uuid>,
    request: Request,
) -> Result<(), ()> {
    // Static files
    if path == "/static/htmx.min.js" {
        return serve_htmx(request);
    }

    // Login page and submission (no auth required)
    if path == "/login" {
        return match method {
            "GET" => serve_login_page(request, None),
            "POST" => handle_login(state, request),
            _ => serve_404(request),
        };
    }

    // Check auth for all other routes
    let session_id = match session_id {
        Some(id) if state.sessions.exists(&id) => id,
        _ => return redirect_to_login(request),
    };

    // Authenticated routes
    match (method, path) {
        ("GET", "/") => serve_main_page(state, &session_id, request),
        ("POST", "/logout") => handle_logout(state, &session_id, request),
        ("GET", "/mailboxes") => handle_mailboxes(state, &session_id, request),
        ("GET", p) if p.starts_with("/mailbox/") && p.ends_with("/emails") => {
            let mailbox_id = p
                .strip_prefix("/mailbox/")
                .and_then(|s| s.strip_suffix("/emails"))
                .unwrap_or("");
            handle_emails(state, &session_id, mailbox_id, request)
        }
        ("GET", p) if p.starts_with("/email/") => {
            let email_id = p.strip_prefix("/email/").unwrap_or("");
            handle_email(state, &session_id, email_id, request)
        }
        _ => serve_404(request),
    }
}

fn html_response(body: String) -> BoxResponse {
    let bytes = body.into_bytes();
    let len = bytes.len();
    Response::from_data(bytes)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
        )
        .with_header(Header::from_bytes(&b"Content-Length"[..], len.to_string()).unwrap())
}

fn serve_htmx(request: Request) -> Result<(), ()> {
    let htmx_js = include_str!("../../static/htmx.min.js");
    let bytes = htmx_js.as_bytes().to_vec();
    let len = bytes.len();
    let response = Response::from_data(bytes)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/javascript"[..]).unwrap(),
        )
        .with_header(Header::from_bytes(&b"Content-Length"[..], len.to_string()).unwrap());
    request.respond(response).map_err(|_| ())
}

fn serve_login_page(request: Request, error: Option<&str>) -> Result<(), ()> {
    let html = templates::login_page(error);
    request.respond(html_response(html)).map_err(|_| ())
}

fn redirect_to_login(request: Request) -> Result<(), ()> {
    // For htmx requests, return the login page directly
    // For regular requests, do a redirect
    let is_htmx = request
        .headers()
        .iter()
        .any(|h| h.field.as_str().to_ascii_lowercase() == "hx-request");

    if is_htmx {
        let html = templates::login_page(None);
        request.respond(html_response(html)).map_err(|_| ())
    } else {
        let response = Response::empty(303)
            .with_header(Header::from_bytes(&b"Location"[..], &b"/login"[..]).unwrap());
        request.respond(response).map_err(|_| ())
    }
}

fn serve_404(request: Request) -> Result<(), ()> {
    let response = Response::from_string("Not Found").with_status_code(404);
    request.respond(response).map_err(|_| ())
}

fn handle_login(state: &Arc<AppState>, mut request: Request) -> Result<(), ()> {
    // Parse form body
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return serve_login_page(request, Some("Failed to read request"));
    }

    let mut username = None;
    let mut password = None;

    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts
            .next()
            .map(|v| urlencoding_decode(v))
            .unwrap_or_default();

        match key {
            "username" => username = Some(value),
            "password" => password = Some(value),
            _ => {}
        }
    }

    let (username, password) = match (username, password) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => (u, p),
        _ => {
            let html = templates::login_page(Some("Username and password required"));
            return request.respond(html_response(html)).map_err(|_| ());
        }
    };

    // Try to authenticate with JMAP server
    match JmapClient::discover(&state.config.jmap.well_known_url, &username, &password) {
        Ok((_session, client)) => {
            let session = Session {
                username: username.clone(),
                password,
                api_url: client.api_url().to_string(),
                account_id: client.account_id().to_string(),
            };

            let session_id = state.sessions.create(session);
            let cookie = make_session_cookie(&session_id);

            let html = templates::main_page(&username);
            let response = html_response(html)
                .with_header(Header::from_bytes(&b"Set-Cookie"[..], cookie.as_bytes()).unwrap());

            request.respond(response).map_err(|_| ())
        }
        Err(e) => {
            let error_msg = format!("Login failed: {}", e);
            let html = templates::login_page(Some(&error_msg));
            request.respond(html_response(html)).map_err(|_| ())
        }
    }
}

fn handle_logout(state: &Arc<AppState>, session_id: &Uuid, request: Request) -> Result<(), ()> {
    state.sessions.remove(session_id);
    let cookie = clear_session_cookie();
    let html = templates::login_page(None);
    let response = html_response(html)
        .with_header(Header::from_bytes(&b"Set-Cookie"[..], cookie.as_bytes()).unwrap());
    request.respond(response).map_err(|_| ())
}

fn serve_main_page(state: &Arc<AppState>, session_id: &Uuid, request: Request) -> Result<(), ()> {
    let username = state
        .sessions
        .get(session_id, |s| s.username.clone())
        .unwrap_or_default();

    let html = templates::main_page(&username);
    request.respond(html_response(html)).map_err(|_| ())
}

fn handle_mailboxes(state: &Arc<AppState>, session_id: &Uuid, request: Request) -> Result<(), ()> {
    let client = match get_client(state, session_id) {
        Some(c) => c,
        None => return redirect_to_login(request),
    };

    match client.get_mailboxes() {
        Ok(mailboxes) => {
            let html = templates::mailbox_list(&mailboxes);
            request.respond(html_response(html)).map_err(|_| ())
        }
        Err(e) => {
            let html = templates::error_fragment(&format!("Failed to load mailboxes: {}", e));
            request.respond(html_response(html)).map_err(|_| ())
        }
    }
}

fn handle_emails(
    state: &Arc<AppState>,
    session_id: &Uuid,
    mailbox_id: &str,
    request: Request,
) -> Result<(), ()> {
    let client = match get_client(state, session_id) {
        Some(c) => c,
        None => return redirect_to_login(request),
    };

    let mailbox_id = urlencoding_decode(mailbox_id);

    match client.query_emails(&mailbox_id, 50) {
        Ok(ids) => match client.get_emails(&ids) {
            Ok(emails) => {
                let html = templates::email_list(&emails);
                request.respond(html_response(html)).map_err(|_| ())
            }
            Err(e) => {
                let html = templates::error_fragment(&format!("Failed to load emails: {}", e));
                request.respond(html_response(html)).map_err(|_| ())
            }
        },
        Err(e) => {
            let html = templates::error_fragment(&format!("Failed to query emails: {}", e));
            request.respond(html_response(html)).map_err(|_| ())
        }
    }
}

fn handle_email(
    state: &Arc<AppState>,
    session_id: &Uuid,
    email_id: &str,
    request: Request,
) -> Result<(), ()> {
    let client = match get_client(state, session_id) {
        Some(c) => c,
        None => return redirect_to_login(request),
    };

    let email_id = urlencoding_decode(email_id);

    match client.get_email(&email_id) {
        Ok(Some(email)) => {
            let html = templates::email_view(&email);
            request.respond(html_response(html)).map_err(|_| ())
        }
        Ok(None) => {
            let html = templates::error_fragment("Email not found");
            request.respond(html_response(html)).map_err(|_| ())
        }
        Err(e) => {
            let html = templates::error_fragment(&format!("Failed to load email: {}", e));
            request.respond(html_response(html)).map_err(|_| ())
        }
    }
}

fn get_client(state: &Arc<AppState>, session_id: &Uuid) -> Option<JmapClient> {
    state.sessions.get(session_id, |s| {
        JmapClient::from_session(
            s.username.clone(),
            s.password.clone(),
            s.api_url.clone(),
            s.account_id.clone(),
        )
    })
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}
