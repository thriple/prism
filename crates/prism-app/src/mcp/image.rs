//! Screenshots for the assistant (brief 21, part 2): shrink, PNG, base64.
//! Small and dependency-free apart from `flate2`, which Prism already has.

use std::io::Write;

/// An RGBA picture, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    /// Shrink to at most `max` pixels wide (keeping the aspect), averaging
    /// the pixels each new one covers. Smaller pictures stay as they are.
    pub fn shrink(&self, max: u32) -> Rgba {
        if self.w <= max || self.w == 0 || self.h == 0 {
            return self.clone();
        }
        let w = max.max(1);
        let h = ((u64::from(self.h) * u64::from(w)) / u64::from(self.w)).max(1) as u32;
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let (y0, y1) = span(y, h, self.h);
            for x in 0..w {
                let (x0, x1) = span(x, w, self.w);
                let mut sum = [0u32; 4];
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        let i = ((sy * self.w + sx) * 4) as usize;
                        for (acc, v) in sum.iter_mut().zip(&self.px[i..i + 4]) {
                            *acc += u32::from(*v);
                        }
                    }
                }
                let n = (y1 - y0) * (x1 - x0);
                px.extend(sum.map(|s| ((s + n / 2) / n) as u8));
            }
        }
        Rgba { w, h, px }
    }

    /// As a PNG file (8-bit RGBA, compressed).
    pub fn png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity(((self.w * 4 + 1) * self.h) as usize);
        for row in self.px.chunks_exact((self.w * 4).max(1) as usize) {
            raw.push(0); // no filter
            raw.extend_from_slice(row);
        }
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = z.write_all(&raw);
        let data = z.finish().unwrap_or_default();
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend(self.w.to_be_bytes());
        ihdr.extend(self.h.to_be_bytes());
        ihdr.extend([8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, no filter, no interlace
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &data);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

/// The source pixels new pixel `i` of `n` covers, of `len`.
fn span(i: u32, n: u32, len: u32) -> (u32, u32) {
    let a = (u64::from(i) * u64::from(len) / u64::from(n)) as u32;
    let b = ((u64::from(i + 1) * u64::from(len)).div_ceil(u64::from(n)) as u32).min(len);
    (a, b.max(a + 1))
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend(crc.sum().to_be_bytes());
}

/// Standard base64, with padding.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(A[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_png_has_its_header_and_pixels() {
        let img = Rgba {
            w: 2,
            h: 2,
            px: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 128],
        };
        let png = img.png();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..24], [0, 0, 0, 2, 0, 0, 0, 2]);
        // The IHDR's checksum, as any PNG reader computes it.
        let mut crc = flate2::Crc::new();
        crc.update(&png[12..29]);
        assert_eq!(&png[29..33], crc.sum().to_be_bytes());
        // The pixels inflate back, a filter byte before each row.
        let len = u32::from_be_bytes(png[33..37].try_into().unwrap()) as usize;
        assert_eq!(&png[37..41], b"IDAT");
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&png[41..41 + len])
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(raw[0], 0);
        assert_eq!(&raw[1..9], &img.px[..8]);
        assert_eq!(&raw[10..18], &img.px[8..]);
        assert!(png.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]));
    }

    #[test]
    fn shrinking_keeps_the_aspect_and_averages() {
        let img = Rgba {
            w: 4,
            h: 2,
            px: [[0u8, 0, 0, 255], [200, 100, 0, 255]]
                .iter()
                .cycle()
                .take(8)
                .flatten()
                .copied()
                .collect(),
        };
        let s = img.shrink(2);
        assert_eq!((s.w, s.h), (2, 1));
        assert_eq!(&s.px[..4], [100, 50, 0, 255]);
        assert_eq!(img.shrink(10), img);
    }
}
