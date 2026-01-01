# Rust JMAP Webmail Client - Implementation Plan

A minimalist webmail client using Rust, htmx, and JMAP.

## Overview

- **Frontend**: htmx (no other JS dependencies), minimal CSS
- **Backend**: Rust with minimal dependencies
- **Protocol**: JMAP (RFC 8620 core, RFC 8621 mail)
- **Session**: UUIDv7 cookies with in-memory credential storage

## Project Structure

```
rust-jmap-webmail/
├── Cargo.toml
├── config.toml
├── src/
│   ├── main.rs              # Entry point, server setup
│   ├── config.rs            # Configuration loading
│   ├── session.rs           # Session/cookie management
│   ├── jmap/
│   │   ├── mod.rs
│   │   ├── client.rs        # JMAP HTTP client
│   │   ├── types.rs         # JMAP request/response types
│   │   └── methods.rs       # JMAP method implementations
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── auth.rs          # Login/logout handlers
│   │   ├── mailbox.rs       # Mailbox listing
│   │   └── email.rs         # Email listing and viewing
│   └── templates/
│       ├── mod.rs
│       ├── base.rs          # Base HTML layout
│       ├── login.rs         # Login form
│       ├── mailbox_list.rs  # Left column mailboxes
│       └── email_view.rs    # Right column email content
├── static/
│   └── htmx.min.js          # htmx library (vendored)
└── rfc/                     # Reference RFCs
```

## Dependencies (Minimal)

```toml
[dependencies]
# HTTP server - tiny, no async runtime required
tiny_http = "0.12"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Config file parsing
toml = "0.8"

# UUID generation (v7 for session cookies)
uuid = { version = "1", features = ["v7"] }

# HTTP client for JMAP (blocking, minimal)
ureq = { version = "2", features = ["json"] }

# Base64 for auth
base64 = "0.22"
```

## Configuration

**config.toml**:
```toml
[server]
listen_addr = "127.0.0.1"
listen_port = 8080

[jmap]
well_known_url = "https://mx.timmydouglas.com/.well-known/jmap"
```

## Implementation Phases

### Phase 1: Project Setup and Configuration

1. Initialize Cargo project with minimal dependencies
2. Create `config.rs` to parse `config.toml`
3. Set up basic HTTP server with `tiny_http`
4. Serve static htmx.js file
5. Create base HTML template with htmx loaded

### Phase 2: Session Management

1. Create `session.rs` with:
   - `Session` struct holding username, JMAP auth token, account ID
   - `SessionStore` using `HashMap<Uuid, Session>` with `RwLock`
   - Cookie parsing/generation utilities
2. Implement session middleware:
   - Extract UUIDv7 from cookie
   - Validate against in-memory store
   - Return `Option<Session>` to handlers

### Phase 3: JMAP Client

1. Implement JMAP discovery:
   - Fetch `.well-known/jmap` endpoint
   - Parse session resource capabilities
   - Extract API URL, account IDs
2. Create JMAP request builder:
   - Method calls with proper structure
   - Authentication header (Basic auth over HTTPS)
3. Implement core methods:
   - `Mailbox/get` - list mailboxes
   - `Email/query` - list emails in mailbox
   - `Email/get` - fetch email content

### Phase 4: Authentication Flow

1. Login page handler (`GET /login`):
   - Render login form with htmx POST
   - Username/password fields
2. Login submission (`POST /login`):
   - Validate credentials against JMAP server
   - On success: create session, set UUIDv7 cookie, redirect
   - On failure: return error fragment
3. Logout handler (`POST /logout`):
   - Remove session from store
   - Clear cookie
   - Redirect to login

### Phase 5: Mailbox UI

1. Main layout (`GET /`):
   - Check session validity
   - If invalid: redirect to login
   - If valid: render two-column layout
2. Mailbox list (`GET /mailboxes`):
   - Fetch mailboxes via JMAP
   - Render as clickable list (htmx targets email list)
   - Show unread counts
3. CSS for two-column layout:
   - Left column: fixed width, scrollable mailbox list
   - Right column: flexible, email content area

### Phase 6: Email Display

1. Email list (`GET /mailbox/{id}/emails`):
   - Query emails in selected mailbox
   - Render as list with subject, from, date
   - htmx click loads email content
2. Email view (`GET /email/{id}`):
   - Fetch full email via JMAP
   - Extract and display headers:
     - From, To, Cc, Subject, Date
   - Render plain text body
   - Handle multipart: prefer text/plain over text/html
3. Plain text rendering:
   - Preserve whitespace formatting
   - Basic quote detection (lines starting with >)

## JMAP Implementation Details

### Discovery (RFC 8620 Section 2)

```
GET /.well-known/jmap
Authorization: Basic <base64(user:pass)>

Response:
{
  "username": "user@example.com",
  "apiUrl": "https://api.example.com/jmap/",
  "primaryAccounts": {
    "urn:ietf:params:jmap:mail": "account-id"
  },
  ...
}
```

### Mailbox/get (RFC 8621 Section 2)

```json
{
  "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
  "methodCalls": [
    ["Mailbox/get", {
      "accountId": "account-id",
      "ids": null
    }, "0"]
  ]
}
```

### Email/query (RFC 8621 Section 4.4)

```json
{
  "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
  "methodCalls": [
    ["Email/query", {
      "accountId": "account-id",
      "filter": { "inMailbox": "mailbox-id" },
      "sort": [{ "property": "receivedAt", "isAscending": false }],
      "limit": 50
    }, "0"]
  ]
}
```

### Email/get (RFC 8621 Section 4.2)

```json
{
  "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
  "methodCalls": [
    ["Email/get", {
      "accountId": "account-id",
      "ids": ["email-id"],
      "properties": ["id", "from", "to", "cc", "subject", "receivedAt", "textBody", "bodyValues"],
      "fetchTextBodyValues": true
    }, "0"]
  ]
}
```

## HTML Templates (htmx patterns)

### Base Layout

```html
<!DOCTYPE html>
<html>
<head>
  <title>Webmail</title>
  <script src="/static/htmx.min.js"></script>
  <style>
    body { margin: 0; font-family: monospace; }
    .container { display: flex; height: 100vh; }
    .sidebar { width: 200px; border-right: 1px solid #ccc; overflow-y: auto; }
    .content { flex: 1; overflow-y: auto; padding: 1rem; }
  </style>
</head>
<body>
  <div class="container">
    <div class="sidebar" hx-get="/mailboxes" hx-trigger="load"></div>
    <div class="content" id="content"></div>
  </div>
</body>
</html>
```

### Login Form

```html
<form hx-post="/login" hx-target="body">
  <input name="username" type="text" placeholder="Email" required>
  <input name="password" type="password" placeholder="Password" required>
  <button type="submit">Login</button>
  <div id="error"></div>
</form>
```

### Mailbox List

```html
<ul>
  <li hx-get="/mailbox/inbox-id/emails" hx-target="#content">
    Inbox (5)
  </li>
  ...
</ul>
```

### Email View

```html
<div class="email">
  <dl>
    <dt>From:</dt><dd>sender@example.com</dd>
    <dt>To:</dt><dd>recipient@example.com</dd>
    <dt>Subject:</dt><dd>Hello World</dd>
    <dt>Date:</dt><dd>2024-01-15 10:30</dd>
  </dl>
  <hr>
  <pre class="body">Plain text email content here...</pre>
</div>
```

## Security Considerations

1. **HTTPS**: JMAP connections must use HTTPS (credentials in header)
2. **Cookie security**: Set `HttpOnly`, `Secure`, `SameSite=Strict`
3. **Session expiry**: Consider implementing session timeout
4. **Memory safety**: Rust provides memory safety guarantees
5. **Input validation**: Sanitize any user input before rendering

## Testing Strategy

1. Unit tests for JMAP type serialization
2. Integration tests with mock JMAP server
3. Manual testing against real JMAP server (mx.timmydouglas.com)

## Future Enhancements (Out of Scope)

- Compose/send emails
- Search functionality
- Sieve filter management (RFC 9661)
- HTML email rendering
- Attachment handling
- Multiple account support
