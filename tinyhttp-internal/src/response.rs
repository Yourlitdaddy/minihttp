use std::{
    collections::HashMap,
    io::{Read, Write},
};

#[derive(Clone, Debug)]
pub struct Response {
    pub headers: HashMap<String, String>,
    pub status_line: String,
    pub body: Option<Vec<u8>>,
    pub mime: Option<String>,
    pub http2: bool,
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

impl Response {
    pub fn new() -> Response {
        Response {
            headers: HashMap::new(),
            status_line: String::new(),
            body: None,
            mime: None,
            http2: false,
        }
    }

    #[allow(dead_code)]
    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn status_line<P: Into<String>>(mut self, line: P) -> Self {
        self.status_line = line.into();
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    pub fn mime<P>(mut self, mime: P) -> Self
    where
        P: Into<String>,
    {
        self.mime = Some(mime.into());
        self
    }

    pub(crate) fn send<P: Read + Write>(&self, sock: &mut P) {
        let line_bytes = self.status_line.as_bytes();
        #[cfg(feature = "log")]
        log::trace!("res status line: {:#?}", self.status_line);

        let mut header_bytes: Vec<u8> = self
            .headers
            .iter()
            .flat_map(|s| [s.0.as_bytes(), s.1.as_bytes()].concat())
            .collect();

        header_bytes.extend(b"\r\n");

        #[cfg(all(feature = "log", debug_assertions))] 
        {
            log::trace!("HEADER AS STR: {}", String::from_utf8(header_bytes.clone()).unwrap());
            log::trace!("STATUS LINE AS STR: {}", std::str::from_utf8(line_bytes).unwrap());
        };

        let full_req: &[u8] = 
            &[
                line_bytes,
                header_bytes.as_slice(),
                self.body.as_ref().unwrap(),
            ].concat();

        sock.write_all(full_req).unwrap();
    }
}
