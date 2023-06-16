use std::collections::HashMap;

/// Struct containing data on a single request.
///
/// parsed_body which is a Option<String> that can contain the body as a String
///
/// body is used when the body of the request is not a String

#[derive(Clone, Debug)]
pub struct Request {
    headers: HashMap<String, String>,
    status_line: Vec<String>,
    body: Vec<u8>,
    wildcard: Option<String>,
    is_http2: bool,
}

#[derive(Clone, Debug)]
pub enum BodyType {
    ASCII(String),
    Bytes(Vec<u8>),
}

impl Request {
    pub fn new(
        raw_body: &[u8],
        raw_headers: Vec<String>,
        status_line: Vec<String>,
        wildcard: Option<String>,
    ) -> Request {
        let mut headers: HashMap<String, String> = HashMap::new();
        #[cfg(feature = "log")]
        log::trace!("Headers: {:#?}", raw_headers);
        for i in raw_headers.iter() {
            let mut iter = i.split(": ");
            let key = iter.next().unwrap();
            let value = iter.next().unwrap();

            /*            match value {
                            Some(v) => println!("{}", v),
                            None => {
                                break;
                            }
                        };
            */
            headers.insert(key.to_string(), value.to_string());
        }

        #[cfg(feature = "log")]
        log::trace!("Request headers: {:?}", headers);

        Request {
            body: raw_body.to_vec(),
            headers,
            status_line,
            wildcard,
            is_http2: false,
        }
    }

    pub(crate) fn set_wildcard(&mut self, w: Option<String>) -> &Self {
        self.wildcard = w;
        self
    }

    /// Get request body as bytes
    pub fn get_raw_body(&self) -> &Vec<u8> {
        &self.body
    }

    /// Get request body as a string
    pub fn get_parsed_body(&self) -> Option<String> {
        if let Ok(s) = std::str::from_utf8(&self.body) {
            Some(s.to_string())
        } else {
            None
        }
    }

    /// Get request headers in a HashMap
    pub fn get_headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Get status line of request
    pub fn get_status_line(&self) -> &Vec<String> {
        &self.status_line
    }

    pub fn get_wildcard(&self) -> Option<&String> {
        self.wildcard.as_ref()
    }

    pub fn get_http2(&self) -> bool {
        self.is_http2
    }

    pub fn set_http2(mut self, w: bool) -> Self {
        self.is_http2 = w;
        self
    }
}
