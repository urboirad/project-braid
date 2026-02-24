use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute a stable SHA1 hash for ROM identity matching.
///
/// This mirrors the Python prototype and returns the full 40-character hex
/// digest so manifests stay compatible across implementations.
pub fn compute_rom_hash(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
