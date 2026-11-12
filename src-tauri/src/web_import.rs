use nabor_core::{Result, MAX_IMAGE_BYTES};
use reqwest::blocking::{Client, Response};
use std::{
    io::Read,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};
use url::{Host, Url};

pub fn validate_url(value: &str) -> Result<Url> {
    if value.len() > 16_384 {
        return Err("Image URL is too long".into());
    }
    let mut url = Url::parse(value).map_err(|_| "Invalid image URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Drop a public HTTP(S) image URL without embedded credentials".into());
    }
    url.set_fragment(None);
    Ok(url)
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let [a, b, _, _] = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.is_multicast()
                || v.is_documentation()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19)))
        }
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let s = v.segments();
            (s[0] & 0xe000) == 0x2000 && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
        }
    }
}
fn addresses(url: &Url) -> Result<Vec<SocketAddr>> {
    let port = url.port_or_known_default().ok_or("Image URL has no port")?;
    let addresses: Vec<_> = match url.host().ok_or("Image URL has no host")? {
        Host::Ipv4(ip) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Host::Domain(host) => (host, port)
            .to_socket_addrs()
            .map_err(|_| "Cannot resolve the image website")?
            .collect(),
    };
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err("Only public website image URLs can be downloaded. For local images, add the file instead.".into());
    }
    Ok(addresses)
}
fn response_bytes(response: Response) -> Result<Vec<u8>> {
    if !response.status().is_success() {
        return Err(format!(
            "Image download failed (HTTP {}). Save the image in Chrome and add the file instead.",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_IMAGE_BYTES as u64)
    {
        return Err("Image exceeds the 50 MB import limit".into());
    }
    let mut bytes = vec![];
    response
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Image download was interrupted")?;
    nabor_core::validated_image_extension(&bytes)?;
    Ok(bytes)
}
pub fn download(source: &str) -> Result<(String, Vec<u8>)> {
    let mut url = validate_url(source)?;
    for redirects in 0..=5 {
        let addrs = addresses(&url)?;
        // Pin checked DNS results, including every redirect. No browser cookies,
        // credentials, or system proxy can route an untrusted web drop to the LAN.
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .no_proxy()
            .resolve_to_addrs(url.host_str().unwrap(), &addrs)
            .build()
            .map_err(|_| "Cannot initialize image download")?;
        let response = client
            .get(url.clone())
            .header("Accept", "image/*")
            .header("User-Agent", "Nabor image import")
            .send()
            .map_err(|_| {
                "Cannot download this image. Save it in Chrome and add the file instead."
            })?;
        if response.status().is_redirection() {
            if redirects == 5 {
                return Err("Image download redirected too many times".into());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|s| s.to_str().ok())
                .ok_or("Image redirect has no destination")?;
            url = validate_url(
                url.join(location)
                    .map_err(|_| "Invalid image redirect")?
                    .as_str(),
            )?;
            continue;
        }
        let filename = url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .filter(|s| !s.is_empty())
            .unwrap_or("Web image")
            .to_string();
        return Ok((filename, response_bytes(response)?));
    }
    Err("Image download failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_web_credentials_and_local_destinations() {
        for value in [
            "file:///C:/secret.png",
            "javascript:alert(1)",
            "https://user:password@example.com/image.png",
        ] {
            assert!(validate_url(value).is_err())
        }
        for value in [
            "http://127.0.0.1/a.png",
            "http://[::1]/a.png",
            "http://169.254.169.254/a.png",
            "http://10.0.0.1/a.png",
        ] {
            assert!(addresses(&validate_url(value).unwrap()).is_err())
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        assert!(!public_ip("100.64.0.1".parse().unwrap()));
    }
    #[test]
    fn rejects_non_image_downloads_and_retains_bytes() {
        fn serve(body: Vec<u8>) -> Response {
            use std::{io::Write, net::TcpListener};
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            });
            Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(format!("http://127.0.0.1:{port}/image"))
                .send()
                .unwrap()
        }
        assert!(response_bytes(serve(b"<html>not an image</html>".to_vec())).is_err());
        let png = nabor_core::encode_clipboard(&[20, 30, 40, 255], 1, 1).unwrap();
        assert_eq!(response_bytes(serve(png.clone())).unwrap(), png);
    }
}
