use crate::jmap::{Email, EmailAddress, Mailbox};

pub fn base_page(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <script src="/static/htmx.min.js"></script>
  <style>
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: monospace; font-size: 14px; background: #fafafa; }}
    .container {{ display: flex; height: 100vh; }}
    .sidebar {{
      width: 200px;
      border-right: 1px solid #ccc;
      background: #f5f5f5;
      display: flex;
      flex-direction: column;
    }}
    .sidebar-header {{
      padding: 0.5rem;
      border-bottom: 1px solid #ccc;
      background: #f0f0f0;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    .sidebar-header .username {{
      font-size: 12px;
      color: #333;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }}
    .mailbox-list {{
      flex: 1;
      overflow-y: scroll;
      padding: 0.5rem 0;
    }}
    .sidebar ul {{ list-style: none; margin: 0; padding: 0; }}
    .sidebar li {{
      padding: 0.5rem 1rem;
      cursor: pointer;
      border-bottom: 1px solid #eee;
    }}
    .sidebar li:hover {{ background: #e8e8e8; }}
    .sidebar li.selected {{ background: #ddd; font-weight: bold; }}
    .sidebar .unread {{ color: #666; font-size: 12px; }}
    .main {{ flex: 1; display: flex; flex-direction: column; overflow: hidden; }}
    .email-list {{
      height: 40%;
      overflow-y: scroll;
      border-bottom: 1px solid #ccc;
    }}
    .email-list table {{ width: 100%; border-collapse: collapse; }}
    .email-list th, .email-list td {{
      padding: 0.5rem;
      text-align: left;
      border-bottom: 1px solid #eee;
    }}
    .email-list th {{ background: #f0f0f0; position: sticky; top: 0; }}
    .email-list tr {{ cursor: pointer; }}
    .email-list tr:hover {{ background: #f5f5f5; }}
    .email-list tr.unread {{ font-weight: bold; }}
    .email-list .subject {{ max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
    .email-list .preview {{ color: #666; font-size: 12px; }}
    .email-view {{
      flex: 1;
      overflow-y: scroll;
      padding: 1rem;
    }}
    .email-view .headers {{ margin-bottom: 1rem; }}
    .email-view .headers dt {{ font-weight: bold; float: left; width: 80px; clear: left; }}
    .email-view .headers dd {{ margin-left: 90px; margin-bottom: 0.25rem; }}
    .email-view .body {{
      white-space: pre-wrap;
      font-family: monospace;
      background: #fff;
      padding: 1rem;
      border: 1px solid #ddd;
    }}
    .login-page {{
      display: flex;
      justify-content: center;
      align-items: center;
      height: 100vh;
      background: #f5f5f5;
    }}
    .login-form {{
      background: #fff;
      padding: 2rem;
      border: 1px solid #ccc;
      width: 300px;
    }}
    .login-form h1 {{ margin: 0 0 1rem 0; font-size: 1.5rem; }}
    .login-form input {{
      display: block;
      width: 100%;
      padding: 0.5rem;
      margin-bottom: 1rem;
      border: 1px solid #ccc;
      font-family: monospace;
    }}
    .login-form button {{
      width: 100%;
      padding: 0.5rem;
      background: #333;
      color: #fff;
      border: none;
      cursor: pointer;
      font-family: monospace;
    }}
    .login-form button:hover {{ background: #555; }}
    .error {{ color: #c00; margin-top: 1rem; }}
    .loading {{ color: #666; font-style: italic; }}
    .logout-btn {{
      padding: 0.25rem 0.5rem;
      background: none;
      border: 1px solid #ccc;
      cursor: pointer;
      color: #666;
      font-family: monospace;
      font-size: 11px;
    }}
    .logout-btn:hover {{ color: #000; }}
  </style>
</head>
<body>
{body}
</body>
</html>"#,
        title = html_escape(title),
        body = body
    )
}

pub fn login_page(error: Option<&str>) -> String {
    let error_html = error
        .map(|e| format!(r#"<div class="error">{}</div>"#, html_escape(e)))
        .unwrap_or_default();

    let body = format!(
        r#"<div class="login-page">
  <form class="login-form" hx-post="/login" hx-target="body" hx-swap="innerHTML">
    <h1>Webmail Login</h1>
    <input name="username" type="text" placeholder="Email address" required autofocus>
    <input name="password" type="password" placeholder="Password" required>
    <button type="submit">Login</button>
    {error_html}
  </form>
</div>"#
    );

    base_page("Login", &body)
}

pub fn main_page(username: &str) -> String {
    let body = format!(
        r#"<div class="container">
  <div class="sidebar">
    <div class="sidebar-header">
      <span class="username">{username}</span>
      <button class="logout-btn" hx-post="/logout" hx-target="body" hx-swap="innerHTML">Logout</button>
    </div>
    <div class="mailbox-list" hx-get="/mailboxes" hx-trigger="load">
      <div class="loading">Loading mailboxes...</div>
    </div>
  </div>
  <div class="main">
    <div class="email-list" id="email-list">
      <div style="padding: 1rem; color: #666;">Select a mailbox</div>
    </div>
    <div class="email-view" id="email-view">
      <div style="color: #666;">Select an email to view</div>
    </div>
  </div>
</div>"#,
        username = html_escape(username)
    );

    base_page("Webmail", &body)
}

pub fn mailbox_list(mailboxes: &[Mailbox]) -> String {
    let mut sorted: Vec<_> = mailboxes.iter().collect();
    sorted.sort_by(|a, b| {
        let role_order = |m: &Mailbox| match m.role.as_deref() {
            Some("inbox") => 0,
            Some("drafts") => 1,
            Some("sent") => 2,
            Some("trash") => 3,
            Some("junk") | Some("spam") => 4,
            Some("archive") => 5,
            _ => 10,
        };
        role_order(a).cmp(&role_order(b)).then(a.name.cmp(&b.name))
    });

    let items: String = sorted
        .iter()
        .map(|m| {
            let unread = if m.unread_emails > 0 {
                format!(r#" <span class="unread">({})</span>"#, m.unread_emails)
            } else {
                String::new()
            };
            format!(
                "<li hx-get=\"/mailbox/{id}/emails\" hx-target=\"#email-list\" hx-swap=\"innerHTML\">{name}{unread}</li>",
                id = html_escape(&m.id),
                name = html_escape(&m.name),
                unread = unread
            )
        })
        .collect();

    format!("<ul>{}</ul>", items)
}

fn email_rows(emails: &[Email], mailbox_id: &str, next_offset: Option<u32>) -> String {
    let rows: String = emails
        .iter()
        .map(|e| {
            let from = e
                .from
                .as_ref()
                .and_then(|f| f.first())
                .map(|a| format_address_short(a))
                .unwrap_or_else(|| "(unknown)".to_string());

            let subject = e
                .subject
                .as_deref()
                .unwrap_or("(no subject)")
                .to_string();

            let date = e
                .received_at
                .as_deref()
                .map(format_date)
                .unwrap_or_default();

            let preview = e.preview.as_deref().unwrap_or("");

            let unread_class = if e.keywords.get("$seen").copied().unwrap_or(false) {
                ""
            } else {
                " class=\"unread\""
            };

            format!(
                "<tr{unread_class} hx-get=\"/email/{id}\" hx-target=\"#email-view\" hx-swap=\"innerHTML\">
  <td>{from}</td>
  <td><span class=\"subject\">{subject}</span><br><span class=\"preview\">{preview}</span></td>
  <td>{date}</td>
</tr>",
                id = html_escape(&e.id),
                from = html_escape(&from),
                subject = html_escape(&subject),
                preview = html_escape(&truncate(preview, 80)),
                date = html_escape(&date),
                unread_class = unread_class
            )
        })
        .collect();

    let load_more = if let Some(offset) = next_offset {
        format!(
            "<tr id=\"loadmore\">\n\
  <td colspan=\"3\" style=\"text-align: center; padding: 1rem;\">\n\
    <button hx-get=\"/mailbox/{mailbox_id}/emails?offset={offset}\" hx-target=\"#loadmore\" hx-swap=\"outerHTML\" style=\"padding: 0.5rem 1rem; cursor: pointer; font-family: monospace; background: #f0f0f0; border: 1px solid #ccc;\">Load More</button>\n\
  </td>\n\
</tr>",
            mailbox_id = html_escape(mailbox_id),
            offset = offset
        )
    } else {
        String::new()
    };

    format!("{}{}", rows, load_more)
}

pub fn email_list(emails: &[Email], mailbox_id: &str, next_offset: Option<u32>) -> String {
    if emails.is_empty() {
        return r#"<div style="padding: 1rem; color: #666;">No emails in this mailbox</div>"#
            .to_string();
    }

    let rows = email_rows(emails, mailbox_id, next_offset);

    format!(
        r#"<table>
<thead><tr><th>From</th><th>Subject</th><th>Date</th></tr></thead>
<tbody>{}</tbody>
</table>"#,
        rows
    )
}

pub fn email_list_rows(emails: &[Email], mailbox_id: &str, next_offset: Option<u32>) -> String {
    email_rows(emails, mailbox_id, next_offset)
}

pub fn email_view(email: &Email) -> String {
    let from = email
        .from
        .as_ref()
        .map(|addrs| format_addresses(addrs))
        .unwrap_or_else(|| "(unknown)".to_string());

    let to = email
        .to
        .as_ref()
        .map(|addrs| format_addresses(addrs))
        .unwrap_or_else(|| "(unknown)".to_string());

    let cc = email.cc.as_ref().map(|addrs| format_addresses(addrs));

    let subject = email
        .subject
        .as_deref()
        .unwrap_or("(no subject)");

    let date = email
        .received_at
        .as_deref()
        .unwrap_or("(unknown date)");

    let body = get_email_body(email);

    let cc_html = cc
        .map(|c| format!("<dt>Cc:</dt><dd>{}</dd>", html_escape(&c)))
        .unwrap_or_default();

    format!(
        r#"<div style="margin-bottom: 0.5rem;">
  <a href="/email/{id}/raw" target="_blank" style="font-size: 12px; color: #666; text-decoration: none; border: 1px solid #ccc; padding: 2px 8px; background: #f5f5f5;">View Raw</a>
</div>
<dl class="headers">
  <dt>From:</dt><dd>{from}</dd>
  <dt>To:</dt><dd>{to}</dd>
  {cc_html}
  <dt>Subject:</dt><dd>{subject}</dd>
  <dt>Date:</dt><dd>{date}</dd>
</dl>
<hr>
<pre class="body">{body}</pre>"#,
        id = html_escape(&email.id),
        from = html_escape(&from),
        to = html_escape(&to),
        cc_html = cc_html,
        subject = html_escape(subject),
        date = html_escape(date),
        body = html_escape(&body)
    )
}

pub fn error_fragment(message: &str) -> String {
    format!(r#"<div class="error">{}</div>"#, html_escape(message))
}

// Helper functions

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_address_short(addr: &EmailAddress) -> String {
    addr.name
        .as_ref()
        .or(addr.email.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn format_addresses(addrs: &[EmailAddress]) -> String {
    addrs
        .iter()
        .map(|a| a.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_date(iso_date: &str) -> String {
    // Simple date formatting - just extract date and time parts
    if let Some(t_pos) = iso_date.find('T') {
        let date = &iso_date[..t_pos];
        let time = iso_date
            .get(t_pos + 1..t_pos + 6)
            .unwrap_or("");
        format!("{} {}", date, time)
    } else {
        iso_date.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

fn get_email_body(email: &Email) -> String {
    // Try to get body from bodyValues using textBody parts
    if let Some(text_body) = &email.text_body {
        for part in text_body {
            if let Some(body_value) = email.body_values.get(&part.part_id) {
                return body_value.value.clone();
            }
        }
    }

    // Fallback to preview
    email
        .preview
        .as_deref()
        .unwrap_or("(no body)")
        .to_string()
}
