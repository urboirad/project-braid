use std::fmt;
use url::{form_urlencoded, Url};

const BRAID_SCHEME: &str = "braid";

#[derive(Debug, Clone)]
pub struct SessionLink {
    pub session_id: String,
    pub signal_url: Option<String>,
    pub manifest_path: Option<String>,
}

#[derive(Debug)]
pub enum LinkError {
    InvalidScheme(String),
    MissingParameters,
    Parse(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::InvalidScheme(s) => write!(f, "unsupported scheme: {s}"),
            LinkError::MissingParameters => write!(f, "missing both 'signal' and 'manifest' parameters in braid link"),
            LinkError::Parse(s) => write!(f, "invalid link: {s}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl SessionLink {
    pub fn parse(uri: &str) -> Result<Self, LinkError> {
        let url = Url::parse(uri).map_err(|e| LinkError::Parse(e.to_string()))?;
        if url.scheme() != BRAID_SCHEME {
            return Err(LinkError::InvalidScheme(url.scheme().to_string()));
        }

        let session_id = if !url.host_str().unwrap_or("").is_empty() {
            url.host_str().unwrap().to_string()
        } else {
            url.path().trim_start_matches('/').to_string()
        };

        let mut signal_url: Option<String> = None;
        let mut manifest_path: Option<String> = None;

        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "signal" => {
                    if !v.is_empty() {
                        signal_url = Some(v.to_string());
                    }
                }
                "manifest" => {
                    if !v.is_empty() {
                        manifest_path = Some(v.to_string());
                    }
                }
                _ => {}
            }
        }

        if signal_url.is_none() && manifest_path.is_none() {
            return Err(LinkError::MissingParameters);
        }

        Ok(SessionLink {
            session_id,
            signal_url,
            manifest_path,
        })
    }

    pub fn to_uri(&self) -> Result<String, LinkError> {
        let mut params = form_urlencoded::Serializer::new(String::new());
        if let Some(ref s) = self.signal_url {
            params.append_pair("signal", s);
        }
        if let Some(ref m) = self.manifest_path {
            params.append_pair("manifest", m);
        }

        let query = params.finish();
        if query.is_empty() {
            return Err(LinkError::MissingParameters);
        }

        let uri = format!("{}://{}?{}", BRAID_SCHEME, self.session_id, query);
        Ok(uri)
    }
}
