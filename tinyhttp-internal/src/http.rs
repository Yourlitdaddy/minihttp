use std::{
    io::{self, BufReader},
    iter::FromIterator,
    path::Path,
    vec,
};

use std::{fs::File, io::Read, io::Write};

use crate::{
    config::{Config, HttpListener},
    request::{Request, RequestError},
    response::Response,
};

#[cfg(feature = "sys")]
use flate2::{write::GzEncoder, Compression};

pub(crate) fn start_http(http: HttpListener) {
    for stream in http.get_stream() {
        let mut conn = stream.unwrap();
        let config = http.config.clone();

        if http.config.ssl && cfg!(feature = "ssl") {
            #[cfg(feature = "ssl")]
            let acpt = http.ssl_acpt.clone().unwrap();
            #[cfg(feature = "ssl")]
            http.pool.execute(move || match acpt.accept(conn) {
                Ok(mut s) => {
                    #[cfg(feature = "log")]
                    log::trace!("parse_request() called");

                    parse_request(&mut s, config);
                }
                Err(s) => {
                    #[cfg(feature = "log")]
                    log::error!("{}", s);
                }
            });
        } else if http.use_pool {
            http.pool.execute(move || {
                #[cfg(feature = "log")]
                log::trace!("parse_request() called");

                parse_request(&mut conn, config);
            });
        } else {
            #[cfg(feature = "log")]
            log::trace!("parse_request() called");

            parse_request(&mut conn, config);
        }

        //conn.write(b"HTTP/1.1 200 OK\r\n").unwrap();
    }
}

fn build_and_parse_req(buf: Vec<u8>) -> Result<Request, RequestError> {
    #[cfg(feature = "log")]
    log::trace!("build_and_parse_req ~~> buf: {:?}", buf);

    let mut safe_http_index = buf.windows(2).enumerate();
    let status_line_index_opt = safe_http_index
        .find(|(_, w)| matches!(*w, b"\r\n"))
        .map(|(i, _)| i);

    let status_line_index = if let Some(status) = status_line_index_opt {
        status
    } else {
        #[cfg(feature = "log")]
        log::error!("failed parsing status line!");

        return Err(RequestError::StatusLineErr);
    };

    let first_header_index = if let Some(first_header_index) = safe_http_index
        .find(|(_, w)| matches!(*w, b"\r\n"))
        .map(|(i, _)| i)
    {
        first_header_index
    } else {
        #[cfg(feature = "log")]
        log::warn!("no headers found!");

        0usize
    };

    #[cfg(feature = "log")]
    log::trace!(
        "STATUS LINE: {:#?}",
        std::str::from_utf8(&buf[..status_line_index])
    );

    #[cfg(feature = "log")]
    log::trace!(
        "FIRST HEADER: {:#?}",
        std::str::from_utf8(&buf[status_line_index + 2..first_header_index])
    );

    let mut headers = Vec::<String>::new();
    let mut headers_index = vec![first_header_index + 2];
    while let Some(header_index) = safe_http_index
        .find(|(_, w)| matches!(*w, b"\r\n"))
        .map(|(i, _)| i + 2)
    {
        #[cfg(feature = "log")]
        log::trace!("header index: {}", header_index);

        let header = std::str::from_utf8(&buf[*headers_index.last().unwrap()..header_index - 2])
            .unwrap()
            .to_lowercase();
        if header.is_empty() {
            break;
        }
        #[cfg(feature = "log")]
        log::trace!("HEADER: {:?}", headers);

        headers_index.push(header_index);
        headers.push(header);
    }

    let iter_status_line = std::str::from_utf8(&buf[..status_line_index]).unwrap();

    //let headers = parse_headers(http.to_string());
    let str_status_line = Vec::from_iter(iter_status_line.split_whitespace());
    let status_line: Vec<String> = str_status_line.iter().map(|i| i.to_string()).collect();
    #[cfg(feature = "log")]
    log::trace!("{:#?}", str_status_line);
    let body_index = buf
        .windows(4)
        .enumerate()
        .find(|(_, w)| matches!(*w, b"\r\n\r\n"))
        .map(|(i, _)| i)
        .unwrap();

    let raw_body = &buf[body_index + 4..];
    //    #[cfg(feature = "log")]
    //    log::debug!(
    //        "BODY (TOP): {:#?}",
    //        std::str::from_utf8(&buf[body_index + 4..]).unwrap()
    //    );
    Ok(Request::new(raw_body, headers, status_line, None))
}

fn build_res(mut req: Request, config: &Config) -> Response {
    let status_line = req.get_status_line();
    #[cfg(feature = "log")]
    log::trace!("build_res -> req_path: {}", status_line[1]);

    match status_line[0].as_str() {
        "GET" => match config.get_routes(&status_line[1]) {
            Some(route) => {
                #[cfg(feature = "log")]
                log::trace!("Found path in routes!");

                if route.wildcard().is_some() {
                    let stat_line = &status_line[1];
                    let split = stat_line
                        .split(&(route.get_path().to_string() + "/"))
                        .last()
                        .unwrap();

                    req.set_wildcard(Some(split.into()));
                };

                /*if route.is_ret_res() {
                    route.get_body_with_res().unwrap()(req_new.to_owned())
                } else {
                    Response::new()
                        .status_line("HTTP/1.1 200 OK\r\n")
                        .body(route.to_body(req_new.to_owned()))
                        .mime("text/plain")
                }*/

                route.to_res(req)
            }

            None => match config.get_mount() {
                Some(old_path) => {
                    let path = old_path.to_owned() + &status_line[1];
                    if Path::new(&path).extension().is_none() && config.get_spa() {
                        let body = read_to_vec(old_path.to_owned() + "/index.html").unwrap();
                        let line = "HTTP/1.1 200 OK\r\n";

                        Response::new()
                            .status_line(line)
                            .body(body)
                            .mime("text/html")
                    } else if Path::new(&path).is_file() {
                        let body = read_to_vec(&path).unwrap();
                        let line = "HTTP/1.1 200 OK\r\n";
                        let mime = mime_guess::from_path(&path)
                            .first_raw()
                            .unwrap_or("text/plain");
                        Response::new().status_line(line).body(body).mime(mime)
                    } else if Path::new(&path).is_dir() {
                        if Path::new(&(path.to_owned() + "/index.html")).is_file() {
                            let body = read_to_vec(path + "/index.html").unwrap();

                            let line = "HTTP/1.1 200 OK\r\n";
                            Response::new()
                                .status_line(line)
                                .body(body)
                                .mime("text/html")
                        } else {
                            Response::new()
                                .status_line("HTTP/1.1 200 OK\r\n")
                                .body(b"<h1>404 Not Found</h1>".to_vec())
                                .mime("text/html")
                        }
                    } else if Path::new(&(path.to_owned() + ".html")).is_file() {
                        let body = read_to_vec(path + ".html").unwrap();
                        let line = "HTTP/1.1 200 OK\r\n";
                        Response::new()
                            .status_line(line)
                            .body(body)
                            .mime("text/html")
                    } else {
                        Response::new()
                            .status_line("HTTP/1.1 404 NOT FOUND\r\n")
                            .body(b"<h1>404 Not Found</h1>".to_vec())
                            .mime("text/html")
                    }
                }

                None => Response::new()
                    .status_line("HTTP/1.1 404 NOT FOUND\r\n")
                    .body(b"<h1>404 Not Found</h1>".to_vec())
                    .mime("text/html"),
            },
        },
        "POST" => match config.post_routes(&status_line[1]) {
            Some(route) => {
                #[cfg(feature = "log")]
                log::debug!("POST");

                if route.wildcard().is_some() {
                    let stat_line = &status_line[1];

                    let split = stat_line
                        .split(&(route.get_path().to_string() + "/"))
                        .last()
                        .unwrap();

                    req.set_wildcard(Some(split.into()));
                };

                route.to_res(req)
            }

            None => Response::new()
                .status_line("HTTP/1.1 404 NOT FOUND\r\n")
                .body(b"<h1>404 Not Found</h1>".to_vec())
                .mime("text/html"),
        },

        _ => Response::new()
            .status_line("HTTP/1.1 404 NOT FOUND\r\n")
            .body(b"<h1>404 Not Found</h1>".to_vec())
            .mime("text/html"),
    }
}

fn parse_request<P: Read + Write>(conn: &mut P, config: Config) {
    let buf = read_stream(conn);
    let request = build_and_parse_req(buf);

    if let Err(e) = request {
        let specific_err = match e {
            RequestError::StatusLineErr => b"failed to parse status line".to_vec(),
            RequestError::HeadersErr => b"failed to parse headers".to_vec(),
        };
        Response::new()
            .mime("text/plain")
            .body(specific_err)
            .send(conn);

        return;
    }

    // NOTE:
    // Err has been handled above
    // Therefore, request should always be Ok

    let request = unsafe { request.unwrap_unchecked() };
    /*#[cfg(feature = "middleware")]
    if let Some(req_middleware) = config.get_request_middleware() {
        req_middleware.lock().unwrap()(&mut request);
    };*/
    let req_headers = request.get_headers();
    let _comp = if config.get_gzip() {
        let i = if req_headers.contains_key("accept-encoding") {
            let tmp_str = req_headers.get("accept-encoding").unwrap();
            let res: Vec<&str> = tmp_str.split(',').map(|s| s.trim()).collect();

            #[cfg(feature = "log")]
            log::trace!("{:#?}", &res);

            res.contains(&"gzip")
        } else {
            false
        };
        i
    } else {
        false
    };

    let mut response = build_res(request, &config);

    let mime = response.mime.as_ref().unwrap();

    let inferred_mime = if let Some(mime_inferred) = infer::get(response.body.as_ref().unwrap()) {
        mime_inferred.mime_type()
    } else {
        mime.as_str()
    };

    if let Some(config_headers) = config.get_headers() {
        response.headers.extend(
            config_headers
                .iter()
                .map(|(i, j)| (i.to_owned(), j.to_owned())),
        );
        //        for (i, j) in config_headers.iter() {
        //            res_brw.headers.insert(i.to_string(), j.to_string());
        //        }
    }

    response.headers.extend([
        (
            "Content-Type: ".to_string(),
            inferred_mime.to_owned() + "\r\n",
        ),
        (
            "tinyhttp: ".to_string(),
            env!("CARGO_PKG_VERSION").to_string() + "\r\n",
        ),
    ]);

    // Only check for 'accept-encoding' header
    // when compression is enabled

    #[cfg(feature = "sys")]
    {
        if _comp {
            let mut writer = GzEncoder::new(Vec::new(), Compression::default());
            writer.write_all(response.body.as_ref().unwrap()).unwrap();
            response.body = Some(writer.finish().unwrap());
            response
                .headers
                .insert("Content-Encoding: ".to_string(), "gzip\r\n".to_string());
        }
    }

    #[cfg(feature = "log")]
    {
        log::trace!(
            "RESPONSE BODY: {:#?},\n RESPONSE HEADERS: {:#?}\n",
            response.body.as_ref().unwrap(),
            response.headers,
        );
    }

    /*#[cfg(feature = "middleware")]
    if let Some(middleware) = config.get_response_middleware() {
        middleware.lock().unwrap()(res_brw.deref_mut());
    }*/

    response.send(conn);
}

pub fn read_to_vec<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fn inner(path: &Path) -> io::Result<Vec<u8>> {
        let file = File::open(path).unwrap();
        let mut content: Vec<u8> = Vec::new();
        let mut reader = BufReader::new(file);
        reader.read_to_end(&mut content).unwrap();
        Ok(content)
    }
    inner(path.as_ref())
}

pub(crate) fn read_stream<P: Read>(stream: &mut P) -> Vec<u8> {
    let buffer_size = 1024;
    let mut request_buffer = vec![];
    loop {
        let mut buffer = vec![0; buffer_size];
        match stream.read(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    break;
                } else if n < buffer_size {
                    request_buffer.append(&mut buffer[..n].to_vec());
                    break;
                } else {
                    request_buffer.append(&mut buffer);
                }
            }
            Err(e) => {
                println!("Error: Could not read string!: {}", e);
                std::process::exit(1);
            }
        }
    }

    request_buffer
}
