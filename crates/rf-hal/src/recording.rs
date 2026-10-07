//! Portable cf32 little endian with versioned JSON frame index. Every entry
//! retains units, provenance, clock domain, sample index and optional epoch.
use crate::Result;
use rf_core::dsp::{Complex, IqFrame};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub offset: u64,
    pub count: usize,
    pub metadata: IqFrame,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub format: String,
    pub frames: Vec<Entry>,
}
fn sidecar(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(".json");
    PathBuf::from(p)
}
pub struct Writer {
    file: File,
    path: PathBuf,
    index: Index,
    offset: u64,
}
impl Writer {
    pub fn create(path: &Path) -> Result<Self> {
        Ok(Self {
            file: File::create(path).map_err(|e| e.to_string())?,
            path: path.into(),
            index: Index {
                version: 1,
                format: "cf32_le".into(),
                frames: vec![],
            },
            offset: 0,
        })
    }
    pub fn append(&mut self, f: &IqFrame) -> Result<()> {
        f.validate()?;
        if self.index.frames.len() >= 100000 {
            return Err("Index limité à 100000 trames".into());
        }
        let mut bytes = Vec::with_capacity(f.samples.len() * 8);
        for z in &f.samples {
            for x in [z.re, z.im] {
                let v = x as f32;
                if !v.is_finite() {
                    return Err("I/Q hors plage float32".into());
                }
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        self.file.write_all(&bytes).map_err(|e| e.to_string())?;
        self.file.flush().map_err(|e| e.to_string())?;
        let mut metadata = f.clone();
        metadata.samples.clear();
        self.index.frames.push(Entry {
            offset: self.offset,
            count: f.samples.len(),
            metadata,
        });
        self.offset += bytes.len() as u64;
        // A temporary sidecar avoids publishing a partially serialized index.
        let mut tmp = sidecar(&self.path).into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(
            &tmp,
            serde_json::to_vec(&self.index).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(tmp, sidecar(&self.path)).map_err(|e| e.to_string())?;
        Ok(())
    }
}
pub struct Reader {
    file: File,
    index: Index,
    next: usize,
}
impl Reader {
    pub fn open(path: &Path) -> Result<Self> {
        let json =
            std::fs::read(sidecar(path)).map_err(|e| format!("Métadonnées I/Q requises : {e}"))?;
        if json.len() > 32 * 1024 * 1024 {
            return Err("Index trop volumineux".into());
        }
        let index: Index = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
        if index.version != 1
            || index.format != "cf32_le"
            || index.frames.is_empty()
            || index.frames.len() > 100000
        {
            return Err("Index/format non pris en charge".into());
        }
        let file = File::open(path).map_err(|e| e.to_string())?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        let mut end = 0;
        for e in &index.frames {
            if e.offset != end || e.count == 0 || e.count > 65536 {
                return Err("Index non contigu ou compte invalide".into());
            }
            end = end
                .checked_add(e.count as u64 * 8)
                .ok_or("Index débordant")?;
        }
        if end != len {
            return Err("RAW tronqué ou index désynchronisé".into());
        }
        Ok(Self {
            file,
            index,
            next: 0,
        })
    }
    pub fn frames(&self) -> usize {
        self.index.frames.len()
    }
    pub fn seek_frame(&mut self, n: usize) -> Result<()> {
        if n >= self.frames() {
            return Err("Trame hors index".into());
        }
        self.next = n;
        Ok(())
    }
    pub fn read_frame(&mut self) -> Result<IqFrame> {
        let entry = self
            .index
            .frames
            .get(self.next)
            .ok_or("Fin de l'enregistrement")?;
        self.file
            .seek(SeekFrom::Start(entry.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; entry.count * 8];
        self.file
            .read_exact(&mut bytes)
            .map_err(|e| e.to_string())?;
        let mut f = entry.metadata.clone();
        f.samples = decode_cf32(&bytes)?;
        f.validate()?;
        self.next += 1;
        Ok(f)
    }
}
pub fn decode_cf32(bytes: &[u8]) -> Result<Vec<Complex>> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(8) || bytes.len() > 65536 * 8 {
        return Err("Longueur cf32 invalide".into());
    }
    bytes
        .chunks_exact(8)
        .map(|b| {
            let re = f32::from_le_bytes(b[..4].try_into().expect("4 bytes")) as f64;
            let im = f32::from_le_bytes(b[4..].try_into().expect("4 bytes")) as f64;
            if !re.is_finite() || !im.is_finite() {
                Err("I/Q non fini".into())
            } else {
                Ok(Complex::new(re, im))
            }
        })
        .collect()
}
