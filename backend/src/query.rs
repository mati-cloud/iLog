//! Log search language, compiled to a parameterised WHERE clause.
//!
//! Terms are ANDed. Every user-supplied string, field names included, is bound
//! as a parameter; nothing from the query is ever spliced into SQL text.
//!
//! ```text
//! timeout                  body contains "timeout" (case-insensitive)
//! "connection reset"       body contains the phrase
//! status:500               attribute equals (also: status:500,502,503)
//! path:/api/*              attribute wildcard, case-insensitive
//! duration_s:>=1.5         numeric compare: > >= < <=
//! user_id:*                attribute present
//! level:error              severity; source:gitlab-nginx is the service name
//! -status:200  -healthz    negate any term
//! ```

use sqlx::{Postgres, QueryBuilder};

#[derive(Debug, PartialEq)]
pub enum Cmp {
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Debug, PartialEq)]
pub enum Matcher {
    Text(String),
    Eq(Vec<String>),
    Like(String),
    Num(Cmp, f64),
    Exists,
}

#[derive(Debug, PartialEq)]
pub struct Term {
    pub negate: bool,
    /// `None` is free text against the body.
    pub field: Option<String>,
    pub matcher: Matcher,
}

struct Token {
    text: String,
    quoted: bool,
    /// Byte offset of the first `:` outside quotes, so `"error: x"` stays text.
    colon: Option<usize>,
}

/// Split on whitespace, keeping "quoted runs" (quotes may start mid-token, as
/// in `path:"/a b"`) together and dropping the quotes.
fn tokenize(q: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur = Token { text: String::new(), quoted: false, colon: None };
    let mut in_quote = false;
    for c in q.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.quoted = true;
            }
            c if c.is_whitespace() && !in_quote => {
                if !cur.text.is_empty() {
                    out.push(std::mem::replace(
                        &mut cur,
                        Token { text: String::new(), quoted: false, colon: None },
                    ));
                }
                cur.quoted = false;
            }
            c => {
                if c == ':' && !in_quote && cur.colon.is_none() {
                    cur.colon = Some(cur.text.len());
                }
                cur.text.push(c);
            }
        }
    }
    if !cur.text.is_empty() {
        out.push(cur);
    }
    out
}

fn is_field_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars().all(|c| c.is_ascii_alphanumeric() || "_.-@".contains(c))
}

pub fn parse(q: &str) -> Vec<Term> {
    tokenize(q)
        .into_iter()
        .map(|Token { text, quoted, colon }| {
            let negate = text.len() > 1 && text.starts_with('-');
            let skip = usize::from(negate);
            let tok = &text[skip..];
            if let Some(i) = colon.filter(|&i| i >= skip) {
                let (k, v) = (&tok[..i - skip], &tok[i - skip + 1..]);
                // `https://x` is a URL, not field "https".
                if is_field_name(k) && !v.is_empty() && !v.starts_with("//") {
                    return Term {
                        negate,
                        field: Some(k.to_string()),
                        matcher: value_matcher(v, quoted),
                    };
                }
            }
            Term { negate, field: None, matcher: Matcher::Text(tok.to_string()) }
        })
        .collect()
}

fn value_matcher(v: &str, quoted: bool) -> Matcher {
    if quoted {
        return Matcher::Eq(vec![v.to_string()]);
    }
    if v == "*" {
        return Matcher::Exists;
    }
    for (op, cmp) in [(">=", Cmp::Ge), ("<=", Cmp::Le), (">", Cmp::Gt), ("<", Cmp::Lt)] {
        if let Some(n) = v.strip_prefix(op).and_then(|n| n.parse::<f64>().ok()) {
            return Matcher::Num(cmp, n);
        }
    }
    if v.contains('*') {
        return Matcher::Like(v.to_string());
    }
    Matcher::Eq(v.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
}

/// `*` becomes `%`; literal `%`, `_` and `\` are escaped.
fn like_pattern(glob: &str) -> String {
    let mut out = String::with_capacity(glob.len() + 2);
    for c in glob.chars() {
        match c {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '*' => out.push('%'),
            c => out.push(c),
        }
    }
    out
}

fn contains_pattern(text: &str) -> String {
    format!("%{}%", like_pattern(text))
}

/// Attribute value as text: a top-level key first (agents emit dotted names
/// such as `http.status_code`), then the same name as a nested path.
fn push_attr(qb: &mut QueryBuilder<'_, Postgres>, field: &str) {
    qb.push("COALESCE(log_attributes->>");
    qb.push_bind(field.to_string());
    qb.push(", log_attributes#>>");
    qb.push_bind(field.split('.').map(String::from).collect::<Vec<_>>());
    qb.push(")");
}

/// Column or attribute expression a field name resolves to.
fn push_field(qb: &mut QueryBuilder<'_, Postgres>, field: &str) {
    match field {
        "level" | "severity" => qb.push("severity_text"),
        "source" | "service" => qb.push("service_name"),
        "message" | "msg" | "body" => qb.push("body"),
        "trace" | "trace_id" => qb.push("trace_id"),
        f => {
            push_attr(qb, f);
            qb
        }
    };
}

fn push_matcher(qb: &mut QueryBuilder<'_, Postgres>, field: Option<&str>, m: &Matcher) {
    let field = field.unwrap_or("body");
    match m {
        Matcher::Text(t) => {
            push_field(qb, field);
            qb.push(" ILIKE ").push_bind(contains_pattern(t));
        }
        Matcher::Like(g) => {
            push_field(qb, field);
            qb.push(" ILIKE ").push_bind(like_pattern(g));
        }
        // `message:foo` means "contains", like free text.
        Matcher::Eq(vals) if matches!(field, "message" | "msg" | "body") => {
            push_field(qb, field);
            let pats: Vec<String> = vals.iter().map(|v| contains_pattern(v)).collect();
            qb.push(" ILIKE ANY(").push_bind(pats).push(")");
        }
        Matcher::Eq(vals) => {
            push_field(qb, field);
            // Levels arrive in mixed case (INFO, error, Warn).
            if matches!(field, "level" | "severity") {
                qb.push(" ILIKE ANY(").push_bind(vals.clone()).push(")");
            } else {
                qb.push(" = ANY(").push_bind(vals.clone()).push(")");
            }
        }
        Matcher::Num(cmp, n) => {
            // CASE, not AND: Postgres does not promise short-circuit evaluation,
            // and casting a non-numeric value would fail the whole query.
            qb.push("CASE WHEN ");
            push_field(qb, field);
            qb.push(" ~ '^-?[0-9]+(\\.[0-9]+)?$' THEN (");
            push_field(qb, field);
            qb.push(")::numeric END ");
            qb.push(match cmp {
                Cmp::Gt => "> ",
                Cmp::Ge => ">= ",
                Cmp::Lt => "< ",
                Cmp::Le => "<= ",
            });
            qb.push_bind(*n).push("::numeric");
        }
        Matcher::Exists => {
            push_field(qb, field);
            qb.push(" IS NOT NULL");
        }
    }
}

/// Append ` AND (...)` for every term.
pub fn push_where(qb: &mut QueryBuilder<'_, Postgres>, terms: &[Term]) {
    for t in terms {
        // COALESCE so a negated term keeps rows that lack the field entirely:
        // `-status:200` should not hide lines that have no status.
        qb.push(if t.negate { " AND NOT COALESCE((" } else { " AND COALESCE((" });
        push_matcher(qb, t.field.as_deref(), &t.matcher);
        qb.push("), false)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(q: &str) -> Term {
        let mut t = parse(q);
        assert_eq!(t.len(), 1, "{q}");
        t.remove(0)
    }

    #[test]
    fn parses_terms() {
        assert_eq!(one("timeout").matcher, Matcher::Text("timeout".into()));
        assert_eq!(one("\"conn reset\"").matcher, Matcher::Text("conn reset".into()));
        let t = one("-status:500,502");
        assert!(t.negate);
        assert_eq!(t.field.as_deref(), Some("status"));
        assert_eq!(t.matcher, Matcher::Eq(vec!["500".into(), "502".into()]));
        assert_eq!(one("duration_s:>=1.5").matcher, Matcher::Num(Cmp::Ge, 1.5));
        assert_eq!(one("path:/api/*").matcher, Matcher::Like("/api/*".into()));
        assert_eq!(one("user_id:*").matcher, Matcher::Exists);
        assert_eq!(one("path:\"/a b*\"").matcher, Matcher::Eq(vec!["/a b*".into()]));
        // Not a field: URL-ish text and a lone dash stay free text.
        assert_eq!(one("https://x").matcher, Matcher::Text("https://x".into()));
        assert_eq!(one("-").matcher, Matcher::Text("-".into()));
        assert_eq!(one("\"error: x\"").matcher, Matcher::Text("error: x".into()));
    }

    #[test]
    fn escapes_like() {
        assert_eq!(like_pattern("50%_a\\*"), "50\\%\\_a\\\\%");
    }

    #[test]
    fn binds_everything() {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT 1 FROM logs WHERE true");
        push_where(&mut qb, &parse("x'; DROP TABLE logs;-- status:>1 -\"a'b\":1"));
        let sql = qb.sql();
        assert!(!sql.contains("DROP"), "{sql}");
        assert!(!sql.contains("a'b"), "{sql}");
    }
}

