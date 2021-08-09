#![deny(warnings)]

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use headers::HeaderMap;
    use http::{Method, StatusCode};
    use std::fs;
    use std::path::PathBuf;

    extern crate static_web_server;
    use static_web_server::{compression, static_files};

    fn root_dir() -> PathBuf {
        PathBuf::from("docker/public/")
    }

    #[tokio::test]
    async fn handle_file() {
        let mut res = static_files::handle(
            &Method::GET,
            &HeaderMap::new(),
            root_dir(),
            "index.html",
            false,
        )
        .await
        .expect("unexpected error response on `handle` function");

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        assert_eq!(res.status(), 200);
        assert_eq!(res.headers()["content-length"], buf.len().to_string());
        assert_eq!(res.headers()["accept-ranges"], "bytes");
        assert!(!res.headers()["last-modified"].is_empty());

        let ctype = &res.headers()["content-type"];

        assert!(
            ctype == "text/html",
            "content-type is not html: {:?}",
            ctype,
        );

        let body = hyper::body::to_bytes(res.body_mut())
            .await
            .expect("unexpected bytes error during `body` convertion");

        assert_eq!(body, buf);
    }

    #[tokio::test]
    async fn handle_file_head() {
        let mut res = static_files::handle(
            &Method::HEAD,
            &HeaderMap::new(),
            root_dir(),
            "index.html",
            false,
        )
        .await
        .expect("unexpected error response on `handle` function");

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        assert_eq!(res.status(), 200);
        assert_eq!(res.headers()["content-length"], buf.len().to_string());
        assert_eq!(res.headers()["accept-ranges"], "bytes");
        assert!(!res.headers()["last-modified"].is_empty());

        let ctype = &res.headers()["content-type"];

        assert!(
            ctype == "text/html",
            "content-type is not html: {:?}",
            ctype,
        );

        let body = hyper::body::to_bytes(res.body_mut())
            .await
            .expect("unexpected bytes error during `body` convertion");

        assert_eq!(body, buf);
    }

    #[tokio::test]
    async fn handle_file_not_found() {
        match static_files::handle(
            &Method::GET,
            &HeaderMap::new(),
            root_dir(),
            "xyz.html",
            false,
        )
        .await
        {
            Ok(_) => {
                panic!("expected a status error 404 but not status 200")
            }
            Err(status) => {
                assert_eq!(status, StatusCode::NOT_FOUND);
            }
        }
    }

    #[tokio::test]
    async fn handle_file_allowed_disallowed_methods() {
        let methods = [
            Method::CONNECT,
            Method::DELETE,
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::PATCH,
            Method::POST,
            Method::PUT,
            Method::TRACE,
        ];
        for method in methods {
            match static_files::handle(&method, &HeaderMap::new(), root_dir(), "index.html", false)
                .await
            {
                Ok(mut res) => match method {
                    // The handle only accepts HEAD or GET request methods
                    Method::GET | Method::HEAD => {
                        let buf = fs::read(root_dir().join("index.html"))
                            .expect("unexpected error during index.html reading");
                        let buf = Bytes::from(buf);

                        assert_eq!(res.status(), 200);
                        assert_eq!(res.headers()["content-length"], buf.len().to_string());
                        assert_eq!(res.headers()["accept-ranges"], "bytes");
                        assert!(!res.headers()["last-modified"].is_empty());

                        let ctype = &res.headers()["content-type"];

                        assert!(
                            ctype == "text/html",
                            "content-type is not html: {:?}",
                            ctype,
                        );

                        let body = hyper::body::to_bytes(res.body_mut())
                            .await
                            .expect("unexpected bytes error during `body` convertion");

                        assert_eq!(body, buf);
                    }
                    _ => {
                        panic!("unexpected response for method {}", method.as_str())
                    }
                },
                Err(status) => {
                    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
                }
            }
        }
    }

    #[tokio::test]
    async fn handle_file_compressions() {
        let encodings = ["gzip", "deflate", "br", "xyz"];
        let method = &Method::GET;

        for enc in encodings {
            let mut headers = HeaderMap::new();
            headers.insert(http::header::ACCEPT_ENCODING, enc.parse().unwrap());

            match static_files::handle(method, &headers, root_dir(), "index.html", false).await {
                Ok(res) => {
                    let res = compression::auto(method, &headers, res)
                        .expect("unexpected bytes error during body compression");

                    let buf = fs::read(root_dir().join("index.html"))
                        .expect("unexpected error during index.html reading");

                    assert_eq!(res.status(), 200);
                    assert_eq!(res.headers()["accept-ranges"], "bytes");
                    assert!(!res.headers()["last-modified"].is_empty());

                    match enc {
                        // The handle only accepts `HEAD` or `GET` request methods
                        "gzip" | "deflate" | "br" => {
                            assert!(res.headers().get("content-length").is_none());
                            assert_eq!(res.headers()["content-encoding"], enc);
                        }
                        _ => {
                            // otherwise the compression doesn't happen because unsupported `accept-encoding`
                            assert_eq!(res.headers()["content-length"], buf.len().to_string());
                            assert!(res.headers().get("content-encoding").is_none());
                        }
                    };

                    let ctype = &res.headers()["content-type"];

                    assert!(
                        ctype == "text/html",
                        "content-type is not html: {:?}",
                        ctype,
                    );
                }
                Err(_) => {
                    panic!("unexpected status error")
                }
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=100-200".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 206);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes 100-200/{}", buf.len())
                );
                assert_eq!(res.headers()["content-length"], "101");
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, &buf[100..=200]);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_out_of_range() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=100-100000".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 416);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes */{}", buf.len())
                );
                assert_eq!(res.headers().get("content-length"), None);
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, "");
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_if_range_too_old() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=100-200".parse().unwrap());
        headers.insert("if-range", "Mon, 18 Nov 1974 00:00:00 GMT".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(res) => {
                assert_eq!(res.status(), 200);
                assert_eq!(res.headers()["content-length"], buf.len().to_string());
                assert_eq!(res.headers().get("content-range"), None);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_suffix() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=100-".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 206);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes 100-{}/{}", buf.len() - 1, buf.len())
                );
                assert_eq!(
                    res.headers()["content-length"],
                    &buf[100..].len().to_string()
                );
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, &buf[100..]);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_suffix_2() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=-100".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 206);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes {}-{}/{}", buf.len() - 100, buf.len() - 1, buf.len())
                );
                assert_eq!(res.headers()["content-length"], "100");
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, &buf[buf.len() - 100..]);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_bad() {
        let mut headers = HeaderMap::new();
        headers.insert("range", "bytes=100-10".parse().unwrap());

        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 416);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes */{}", buf.len())
                );
                assert_eq!(res.headers().get("content-length"), None);
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, "");
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_bad_2() {
        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        let mut headers = HeaderMap::new();
        headers.insert(
            "range",
            format!("bytes=-{}", buf.len() + 1).parse().unwrap(),
        );

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 416);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes */{}", buf.len())
                );
                assert_eq!(res.headers().get("content-length"), None);
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, "");
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_bad_3() {
        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        let mut headers = HeaderMap::new();
        // Range::Unbounded for beginning and end
        headers.insert("range", "bytes=".parse().unwrap());

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 200);
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, buf);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_exclude_file_size() {
        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        let mut headers = HeaderMap::new();
        // range including end of file (non-inclusive result)
        headers.insert("range", format!("bytes=100-{}", buf.len()).parse().unwrap());

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 206);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes 100-{}/{}", buf.len() - 1, buf.len())
                );
                assert_eq!(
                    res.headers()["content-length"],
                    format!("{}", buf.len() - 100)
                );
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, &buf[100..=buf.len() - 1]);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }

    #[tokio::test]
    async fn handle_byte_ranges_exclude_file_size_2() {
        let buf = fs::read(root_dir().join("index.html"))
            .expect("unexpected error during index.html reading");
        let buf = Bytes::from(buf);

        let mut headers = HeaderMap::new();
        // range with 1 byte to end yields same result as above. (inclusive result)
        headers.insert(
            "range",
            format!("bytes=100-{}", buf.len() - 1).parse().unwrap(),
        );

        match static_files::handle(&Method::GET, &headers, root_dir(), "index.html", false).await {
            Ok(mut res) => {
                assert_eq!(res.status(), 206);
                assert_eq!(
                    res.headers()["content-range"],
                    format!("bytes 100-{}/{}", buf.len() - 1, buf.len())
                );
                assert_eq!(
                    res.headers()["content-length"],
                    format!("{}", buf.len() - 100)
                );
                let body = hyper::body::to_bytes(res.body_mut())
                    .await
                    .expect("unexpected bytes error during `body` convertion");
                assert_eq!(body, &buf[100..=buf.len() - 1]);
            }
            Err(_) => {
                panic!("expected a normal response rather than a status error")
            }
        }
    }
}
