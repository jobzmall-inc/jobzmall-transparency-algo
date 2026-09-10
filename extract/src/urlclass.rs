use regex::Regex;
use std::sync::LazyLock;
use url::Url;

/// The one personal-mailbox vocabulary: [`classify_apply`] reads it for the
/// apply target, [`personal_mail_in_text`] for a mailbox named in the body.
const PERSONAL_MAIL: &[&str] = &[
    "gmail.com",
    "yahoo.com",
    "hotmail.com",
    "outlook.com",
    "icloud.com",
    "aol.com",
    "proton.me",
    "protonmail.com",
    "live.com",
    "me.com",
];

static PERSONAL_IN_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    let alternates = PERSONAL_MAIL
        .iter()
        .copied()
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"@({alternates})\b")).expect("personal-mail pattern")
});

pub fn personal_mail_in_text(text: &str) -> Option<String> {
    PERSONAL_IN_TEXT
        .captures(text)
        .map(|c| c[1].to_ascii_lowercase())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtsProvider {
    Greenhouse,
    Lever,
    Ashby,
    Workable,
    Workday,
    Icims,
    SmartRecruiters,
    Jobvite,
    Taleo,
    Bamboo,
    Rippling,
    Gem,
}

impl AtsProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Greenhouse => "greenhouse",
            Self::Lever => "lever",
            Self::Ashby => "ashby",
            Self::Workable => "workable",
            Self::Workday => "workday",
            Self::Icims => "icims",
            Self::SmartRecruiters => "smartrecruiters",
            Self::Jobvite => "jobvite",
            Self::Taleo => "taleo",
            Self::Bamboo => "bamboohr",
            Self::Rippling => "rippling",
            Self::Gem => "gem",
        }
    }

    fn from_host(host: &str) -> Option<Self> {
        let h = host.to_ascii_lowercase();
        if h.contains("greenhouse.io") || h.contains("greenhouse.com") {
            return Some(Self::Greenhouse);
        }
        if h.contains("lever.co") {
            return Some(Self::Lever);
        }
        if h.contains("ashbyhq.com") {
            return Some(Self::Ashby);
        }
        if h.contains("workable.com") {
            return Some(Self::Workable);
        }
        if h.contains("myworkdayjobs.com") || h.contains("workday.com") {
            return Some(Self::Workday);
        }
        if h.contains("icims.com") {
            return Some(Self::Icims);
        }
        if h.contains("smartrecruiters.com") {
            return Some(Self::SmartRecruiters);
        }
        if h.contains("jobvite.com") {
            return Some(Self::Jobvite);
        }
        if h.contains("taleo.net") {
            return Some(Self::Taleo);
        }
        if h.contains("bamboohr.com") {
            return Some(Self::Bamboo);
        }
        if h.contains("rippling.com") {
            return Some(Self::Rippling);
        }
        if h.contains("gem.com") {
            return Some(Self::Gem);
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginClass {
    Ats { provider: AtsProvider, host: String },
    Career { host: String },
    Jobzmall,
    Unknown,
}

impl OriginClass {
    /// The host this origin resolved to, when there was one.
    pub fn host(&self) -> Option<&str> {
        match self {
            Self::Ats { host, .. } | Self::Career { host } => Some(host),
            Self::Jobzmall | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyKind {
    PersonalEmail { domain: String },
    CorporateEmail { domain: String },
    Url { host: String },
    Text(String),
}

pub fn classify_origin(origin_url: &str, origin_name: &str, jobzmall_url: &str) -> OriginClass {
    if host_of(jobzmall_url).as_deref().is_some_and(is_jobzmall)
        && host_of(origin_url).is_none()
        && origin_name.is_empty()
    {
        return OriginClass::Jobzmall;
    }
    if let Some(host) = host_of(origin_url) {
        if is_jobzmall(&host) {
            return OriginClass::Jobzmall;
        }
        if let Some(provider) = AtsProvider::from_host(&host) {
            return OriginClass::Ats { provider, host };
        }
        if is_career_host(&host, origin_url) {
            return OriginClass::Career { host };
        }
    }
    let name = origin_name.to_ascii_lowercase();
    if name.contains("greenhouse") {
        return OriginClass::Ats {
            provider: AtsProvider::Greenhouse,
            host: name,
        };
    }
    if name.contains("jobzmall") || name.contains("posted on jobzmall") {
        return OriginClass::Jobzmall;
    }
    if name.contains("career") {
        return OriginClass::Career {
            host: origin_name.to_string(),
        };
    }
    OriginClass::Unknown
}

pub fn classify_apply(dest: &str) -> ApplyKind {
    let trimmed = dest.trim();
    if let Some(email) = extract_email(trimmed) {
        let domain = email.split('@').nth(1).unwrap_or("").to_ascii_lowercase();
        if PERSONAL_MAIL.contains(&domain.as_str()) {
            return ApplyKind::PersonalEmail { domain };
        }
        if !domain.is_empty() {
            return ApplyKind::CorporateEmail { domain };
        }
    }
    if let Some(host) = host_of(trimmed) {
        return ApplyKind::Url { host };
    }
    ApplyKind::Text(trimmed.to_string())
}

fn host_of(raw: &str) -> Option<String> {
    let candidate = if raw.contains("://") {
        raw.to_string()
    } else if raw.starts_with("www.") || raw.contains('.') && !raw.contains(' ') {
        format!("https://{raw}")
    } else {
        return None;
    };
    Url::parse(&candidate)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

fn is_jobzmall(host: &str) -> bool {
    host == "jobzmall.com"
        || host.ends_with(".jobzmall.com")
        || host == "www.jobzmall.com"
        || host == "app.jobzmall.com"
}

fn is_career_host(host: &str, url: &str) -> bool {
    let h = host.to_ascii_lowercase();
    let path = url.to_ascii_lowercase();
    h.starts_with("careers.")
        || h.starts_with("jobs.")
        || h.contains(".careers.")
        || path.contains("/careers")
        || path.contains("/jobs/")
}

fn is_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '+' || c == '-'
}

fn is_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

fn extract_email(raw: &str) -> Option<String> {
    let at = raw.find('@')?;
    // Walk out by characters: `i + 1` past a multibyte char lands inside a
    // codepoint, where slicing panics.
    let start = raw[..at]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_local_char(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let end = raw[at + 1..]
        .char_indices()
        .find(|(_, c)| !is_domain_char(*c))
        .map_or(raw.len(), |(i, _)| at + 1 + i);
    let email = raw[start..end].to_ascii_lowercase();
    if email.contains('@') && email.split('@').nth(1).is_some_and(|d| d.contains('.')) {
        Some(email)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greenhouse_is_ats() {
        let c = classify_origin(
            "https://job-boards.greenhouse.io/figma/jobs/1",
            "Greenhouse · Figma",
            "https://www.jobzmall.com/jobs/x",
        );
        assert!(matches!(
            c,
            OriginClass::Ats {
                provider: AtsProvider::Greenhouse,
                ..
            }
        ));
    }

    #[test]
    fn gmail_apply_is_personal() {
        assert!(matches!(
            classify_apply("hr.recruiting@gmail.com"),
            ApplyKind::PersonalEmail { .. }
        ));
    }

    #[test]
    fn stripe_careers_is_career() {
        let c = classify_origin(
            "https://stripe.com/jobs/listing/staff-software-engineer/0000",
            "careers.stripe.com",
            "https://www.jobzmall.com/jobs/x",
        );
        assert!(matches!(c, OriginClass::Career { .. }));
        assert_eq!(c.host(), Some("stripe.com"));
    }

    #[test]
    fn an_address_after_a_multibyte_character_does_not_panic() {
        assert!(matches!(
            classify_apply("Écrire — hr@gmail.com"),
            ApplyKind::PersonalEmail { .. }
        ));
        assert!(matches!(
            classify_apply("контакт: hr@gmail.com"),
            ApplyKind::PersonalEmail { .. }
        ));
        assert!(matches!(
            classify_apply("hr@gmail.com — écrivez-nous"),
            ApplyKind::PersonalEmail { .. }
        ));
    }

    #[test]
    fn corporate_mail_is_not_personal() {
        assert!(matches!(
            classify_apply("talent@acme.com"),
            ApplyKind::CorporateEmail { .. }
        ));
    }

    #[test]
    fn text_scan_shares_the_apply_vocabulary() {
        for domain in PERSONAL_MAIL {
            let body = format!("write to recruiting@{domain} today");
            assert_eq!(
                personal_mail_in_text(&body).as_deref(),
                Some(*domain),
                "{domain} missing from the text scan"
            );
        }
        assert!(personal_mail_in_text("talent@acme.com").is_none());
        assert!(personal_mail_in_text("hi@gmail.community").is_none());
    }
}
